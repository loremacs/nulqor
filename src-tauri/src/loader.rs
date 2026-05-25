//! Extension loader — discover → lint → dependency-sort → activate (DESIGN.md §2.1, BUILD_PLAN §1.7).
//!
//! Phase 1: static compilation. Extensions are registered by id in the loader's factory
//! map before `scan_and_load` is called. The loader reads `extension.toml` from disk,
//! lints it, verifies the id matches the registered factory, topologically sorts by
//! `requires`, and calls `activate()` in dependency order.
//!
//! A broken extension (lint failure, missing factory, dependency cycle) fails before
//! any extension activates. This is the gate DESIGN.md §2.1 requires.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use semver::Version;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{
    CapabilityDecl, CommandDecl, CommandId, EventId, EventPattern, ExtensionKind,
    ExtensionManifest, Permission,
};

// ---------------------------------------------------------------------------
// Factory type
// ---------------------------------------------------------------------------

type Factory = Box<dyn Fn(ExtensionManifest) -> Arc<dyn Extension> + Send + Sync>;

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

pub struct Loader {
    factories: HashMap<String, Factory>,
}

impl Loader {
    pub fn new() -> Self {
        Self { factories: HashMap::new() }
    }

    /// Register a static extension factory by extension id.
    /// The factory receives the parsed manifest so it can store it internally.
    pub fn register(
        &mut self,
        id: &str,
        factory: impl Fn(ExtensionManifest) -> Arc<dyn Extension> + Send + Sync + 'static,
    ) {
        self.factories.insert(id.to_owned(), Box::new(factory));
    }

    /// Discover, lint, sort, and activate extensions in `extensions_dir`.
    /// `root` is the repo root used by the linter for depth calculations.
    /// When `enabled_filter` is `Some`, only listed ids (plus transitive `requires`) load.
    pub fn scan_and_load(
        &self,
        extensions_dir: &Path,
        root: &Path,
        ctx: &CoreContext,
        enabled_filter: Option<&HashSet<String>>,
    ) -> Result<(), CoreError> {
        // 1. Discover extension directories
        let ext_dirs = self.discover(extensions_dir)?;

        // 2. Parse and lint each manifest
        let mut manifests: Vec<(PathBuf, ExtensionManifest)> = Vec::new();
        for ext_dir in &ext_dirs {
            let toml_path = ext_dir.join("extension.toml");

            // Run the linter — fail before any activation on any error
            let lint_errors = nulqor_lint::lint_extension_dir(ext_dir, root);
            if !lint_errors.is_empty() {
                return Err(CoreError::Linter(lint_errors.join("\n")));
            }

            // Parse the manifest
            let raw = nulqor_lint::parse_manifest(&toml_path)
                .map_err(|e| CoreError::Linter(e))?;

            let manifest = convert_manifest(raw).map_err(|e| CoreError::Linter(e))?;
            manifests.push((ext_dir.clone(), manifest));
        }

        if let Some(enabled) = enabled_filter {
            let expanded = expand_enabled_with_deps(&manifests, enabled)?;
            manifests.retain(|(_, m)| expanded.contains(&m.id));
            eprintln!(
                "[LOADER] startup profile: loading {} of {} discovered extensions",
                manifests.len(),
                ext_dirs.len()
            );
        }

        // 3. Topological sort by `requires`
        let sorted_ids = self.topo_sort(&manifests)?;

        // 4. Activate in dependency order
        for id in sorted_ids {
            let (_, manifest) = manifests
                .iter()
                .find(|(_, m)| m.id == id)
                .expect("id from topo_sort must exist in manifests");

            let factory = self.factories.get(&id).ok_or_else(|| {
                CoreError::ExtensionNotFound(format!(
                    "'{id}' has extension.toml but no registered factory"
                ))
            })?;

            let ext = factory(manifest.clone());

            // Register the extension's declared scopes with the capability layer
            ctx.caps.register_scopes(
                &manifest.id,
                manifest.fs_scopes.clone(),
                manifest.http_hosts.clone(),
            );

            // Verify declared commands against version manager
            for cmd in &manifest.commands {
                ctx.versions.register_contract(&cmd.id.base_key(), cmd.id.version);
            }

            // Activate
            ext.activate(ctx).map_err(|e| CoreError::Activation {
                ext_id: id.clone(),
                source: Box::new(e),
            })?;

            ctx.versions.record_loaded(&manifest.id, manifest.version.clone());
            eprintln!("[LOADER] activated '{}'", manifest.id);
        }

        Ok(())
    }

    // ── Internals ──────────────────────────────────────────────────────────

    fn discover(&self, extensions_dir: &Path) -> Result<Vec<PathBuf>, CoreError> {
        if !extensions_dir.exists() {
            return Ok(vec![]);
        }
        let mut dirs = Vec::new();
        let entries = std::fs::read_dir(extensions_dir)
            .map_err(|e| CoreError::Io(format!("cannot read extensions/: {e}")))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("extension.toml").exists() {
                dirs.push(path);
            }
        }
        dirs.sort(); // deterministic order before topo sort
        Ok(dirs)
    }

    fn topo_sort(
        &self,
        manifests: &[(PathBuf, ExtensionManifest)],
    ) -> Result<Vec<String>, CoreError> {
        let id_set: HashSet<&str> = manifests.iter().map(|(_, m)| m.id.as_str()).collect();
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut in_stack = HashSet::new();

        let index: HashMap<&str, &ExtensionManifest> =
            manifests.iter().map(|(_, m)| (m.id.as_str(), m)).collect();

        for (_, manifest) in manifests {
            if !visited.contains(manifest.id.as_str()) {
                self.visit(
                    &manifest.id,
                    &index,
                    &id_set,
                    &mut visited,
                    &mut in_stack,
                    &mut result,
                )?;
            }
        }

        Ok(result)
    }

    fn visit(
        &self,
        id: &str,
        index: &HashMap<&str, &ExtensionManifest>,
        id_set: &HashSet<&str>,
        visited: &mut HashSet<String>,
        in_stack: &mut HashSet<String>,
        result: &mut Vec<String>,
    ) -> Result<(), CoreError> {
        if in_stack.contains(id) {
            return Err(CoreError::DependencyCycle(id.to_owned()));
        }
        if visited.contains(id) {
            return Ok(());
        }

        in_stack.insert(id.to_owned());

        if let Some(manifest) = index.get(id) {
            for dep in &manifest.requires {
                if !id_set.contains(dep.as_str()) {
                    return Err(CoreError::ExtensionNotFound(format!(
                        "'{id}' requires '{dep}' but it is not installed"
                    )));
                }
                self.visit(dep, index, id_set, visited, in_stack, result)?;
            }
        }

        in_stack.remove(id);
        visited.insert(id.to_owned());
        result.push(id.to_owned());
        Ok(())
    }
}

/// Expand an explicit enable list to include all transitive `requires` dependencies.
pub fn expand_enabled_with_deps(
    manifests: &[(PathBuf, ExtensionManifest)],
    enabled: &HashSet<String>,
) -> Result<HashSet<String>, CoreError> {
    let index: HashMap<&str, &ExtensionManifest> =
        manifests.iter().map(|(_, m)| (m.id.as_str(), m)).collect();

    let mut expanded = HashSet::new();
    let mut stack: Vec<&String> = enabled.iter().collect();

    while let Some(id) = stack.pop() {
        if !expanded.insert(id.clone()) {
            continue;
        }
        let manifest = index.get(id.as_str()).ok_or_else(|| {
            CoreError::ExtensionNotFound(format!(
                "enabled extension '{id}' is not installed"
            ))
        })?;
        for dep in &manifest.requires {
            if !index.contains_key(dep.as_str()) {
                return Err(CoreError::ExtensionNotFound(format!(
                    "enabled extension '{id}' requires '{dep}' but it is not installed"
                )));
            }
            stack.push(dep);
        }
    }

    Ok(expanded)
}

// ---------------------------------------------------------------------------
// Manifest conversion: nulqor_lint::Manifest → ExtensionManifest
// ---------------------------------------------------------------------------

fn convert_manifest(raw: nulqor_lint::Manifest) -> Result<ExtensionManifest, String> {
    let ext = raw.extension;

    let version = Version::parse(&ext.version)
        .map_err(|e| format!("extension.version: {e}"))?;
    let kind = ExtensionKind::parse(&ext.kind)
        .ok_or_else(|| format!("unknown kind '{}'", ext.kind))?;
    let schema_version = Version::parse(&ext.schema_version)
        .map_err(|e| format!("extension.schema-version: {e}"))?;
    let min_core = Version::parse(&ext.min_core)
        .map_err(|e| format!("extension.min-core: {e}"))?;

    let commands: Result<Vec<CommandDecl>, String> = raw
        .commands
        .into_iter()
        .map(|c| {
            let id = CommandId::parse(&c.id)?;
            let perm = match c.permission.as_str() {
                "read" => Permission::Read,
                "write" => Permission::Write,
                "destructive" => Permission::Destructive,
                "system" => Permission::System,
                other => return Err(format!("unknown permission '{other}'")),
            };
            Ok(CommandDecl {
                id,
                owner: c.owner,
                input_schema: c.input_schema,
                output_schema: c.output_schema,
                callable_by: c.callable_by,
                permission: perm,
            })
        })
        .collect();

    let publishes: Result<Vec<EventId>, String> = raw
        .publishes
        .into_iter()
        .map(|p| EventId::parse(&p.id))
        .collect();

    let subscribes: Vec<EventPattern> = raw
        .subscribes
        .into_iter()
        .map(|s| EventPattern {
            namespace: s.namespace,
            name: s.name,
            version_range: None,
        })
        .collect();

    let provides: Vec<CapabilityDecl> = raw
        .provides
        .into_iter()
        .map(|p| CapabilityDecl {
            capability: p.capability,
            instance: p.instance,
            contract: p.contract,
        })
        .collect();

    Ok(ExtensionManifest {
        id: ext.id,
        version,
        kind,
        api_version: ext.api_version,
        schema_version,
        min_core,
        requires: ext.requires,
        optional: ext.optional,
        provides,
        commands: commands?,
        publishes: publishes?,
        subscribes,
        fs_scopes: ext.fs_scopes,
        http_hosts: ext.http_hosts,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "nulqor-loader-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn valid_toml(id: &str) -> String {
        format!(
            r#"[extension]
id = "{id}"
version = "0.1.0"
kind = "Service"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
"#
        )
    }

    fn make_context() -> CoreContext {
        use crate::{
            capability::{CapabilityRegistry, Capabilities},
            commands::CommandRegistry,
            context::InMemoryConfigStore,
            events::EventBus,
            permission::PermissionGate,
            runtime::Runtime,
            version::VersionManager,
        };
        let perms = Arc::new(PermissionGate::new());
        CoreContext {
            bus: Arc::new(EventBus::new()),
            commands: Arc::new(CommandRegistry::new(perms.clone())),
            versions: Arc::new(VersionManager::new()),
            permissions: perms,
            caps: Arc::new(Capabilities::new()),
            capability_registry: Arc::new(CapabilityRegistry::new()),
            runtime: Arc::new(Runtime::new()),
            config: Arc::new(InMemoryConfigStore::new()),
        }
    }

    /// A no-op extension for loader tests.
    struct NoopExt(ExtensionManifest);
    impl Extension for NoopExt {
        fn manifest(&self) -> &ExtensionManifest {
            &self.0
        }
        fn activate(&self, _ctx: &CoreContext) -> Result<(), CoreError> {
            Ok(())
        }
    }

    #[test]
    fn loads_valid_extension() {
        let root = tmp_dir();
        write(&root, "extensions/svc/extension.toml", &valid_toml("svc"));
        let ctx = make_context();
        let mut loader = Loader::new();
        loader.register("svc", |m| Arc::new(NoopExt(m)));
        let result = loader.scan_and_load(&root.join("extensions"), &root, &ctx, None);
        assert!(result.is_ok(), "expected ok, got: {result:?}");
    }

    #[test]
    fn rejects_broken_extension_before_activation() {
        let root = tmp_dir();
        // Broken: id has spaces
        write(
            &root,
            "extensions/bad/extension.toml",
            r#"[extension]
id = "Bad Extension"
version = "0.1.0"
kind = "Service"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
"#,
        );
        let ctx = make_context();
        let mut loader = Loader::new();
        loader.register("bad", |m| Arc::new(NoopExt(m)));
        let result = loader.scan_and_load(&root.join("extensions"), &root, &ctx, None);
        assert!(
            matches!(result, Err(CoreError::Linter(_))),
            "expected Linter error, got: {result:?}"
        );
    }

    #[test]
    fn topo_sort_respects_requires() {
        let root = tmp_dir();
        write(&root, "extensions/a/extension.toml", &valid_toml("a"));
        write(
            &root,
            "extensions/b/extension.toml",
            r#"[extension]
id = "b"
version = "0.1.0"
kind = "Service"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
requires = ["a"]
"#,
        );
        let ctx = make_context();
        let mut loader = Loader::new();
        let order = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let o1 = order.clone();
        loader.register("a", move |m| {
            o1.lock().unwrap().push("a".into());
            Arc::new(NoopExt(m))
        });
        let o2 = order.clone();
        loader.register("b", move |m| {
            o2.lock().unwrap().push("b".into());
            Arc::new(NoopExt(m))
        });
        loader.scan_and_load(&root.join("extensions"), &root, &ctx, None).unwrap();
        let loaded = order.lock().unwrap().clone();
        assert_eq!(loaded, vec!["a", "b"], "b must load after a");
    }

    #[test]
    fn enabled_filter_loads_subset_and_deps() {
        let root = tmp_dir();
        write(&root, "extensions/a/extension.toml", &valid_toml("a"));
        write(
            &root,
            "extensions/b/extension.toml",
            r#"[extension]
id = "b"
version = "0.1.0"
kind = "Service"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
requires = ["a"]
"#,
        );
        write(&root, "extensions/c/extension.toml", &valid_toml("c"));
        let ctx = make_context();
        let mut loader = Loader::new();
        let loaded = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let l1 = loaded.clone();
        loader.register("a", move |m| {
            l1.lock().unwrap().push("a".into());
            Arc::new(NoopExt(m))
        });
        let l2 = loaded.clone();
        loader.register("b", move |m| {
            l2.lock().unwrap().push("b".into());
            Arc::new(NoopExt(m))
        });
        loader.register("c", move |m| Arc::new(NoopExt(m)));
        let enabled: HashSet<String> = ["b".into()].into_iter().collect();
        loader
            .scan_and_load(&root.join("extensions"), &root, &ctx, Some(&enabled))
            .unwrap();
        let ids = loaded.lock().unwrap().clone();
        assert_eq!(ids, vec!["a", "b"], "b must pull in required a, skip c");
    }

    #[test]
    fn missing_factory_fails_loud() {
        let root = tmp_dir();
        write(&root, "extensions/orphan/extension.toml", &valid_toml("orphan"));
        let ctx = make_context();
        let loader = Loader::new(); // no factory registered
        let result = loader.scan_and_load(&root.join("extensions"), &root, &ctx, None);
        assert!(
            matches!(result, Err(CoreError::ExtensionNotFound(_))),
            "expected ExtensionNotFound, got: {result:?}"
        );
    }
}
