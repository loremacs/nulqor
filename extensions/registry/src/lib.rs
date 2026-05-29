//! Registry extension — extension graph, manifest introspection, command catalog.
//!
//! Commands:
//!   - `extensions:list@1`   — all discovered extensions + enabled flag
//!   - `extensions:graph@1` — dependency nodes and edges
//!   - `commands:catalog@1` — runtime command registry snapshot

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::startup_config::load_startup_config;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

pub struct RegistryExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
    extensions_dir: PathBuf,
    root: PathBuf,
}

impl RegistryExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        let root = resolve_workspace_root();
        let extensions_dir = root.join("extensions");
        Self {
            manifest,
            extensions_dir,
            root,
        }
    }

    fn scan_manifests(&self) -> Result<Vec<ExtensionRecord>, CoreError> {
        if !self.extensions_dir.exists() {
            return Ok(vec![]);
        }

        let mut records = Vec::new();
        let mut entries: Vec<_> = std::fs::read_dir(&self.extensions_dir)
            .map_err(|e| CoreError::Io(format!("read extensions/: {e}")))?
            .flatten()
            .filter(|e| e.path().is_dir() && e.path().join("extension.toml").exists())
            .collect();
        entries.sort_by_key(|e| e.file_name());

        for entry in entries {
            let ext_dir = entry.path();
            let toml_path = ext_dir.join("extension.toml");
            let raw = nulqor_lint::parse_manifest(&toml_path)
                .map_err(|e| CoreError::Linter(e))?;
            let meta = &raw.extension;
            records.push(ExtensionRecord {
                id: meta.id.clone(),
                version: meta.version.clone(),
                kind: meta.kind.clone(),
                requires: meta.requires.clone(),
                optional: meta.optional.clone(),
                command_count: raw.commands.len(),
                fs_scopes: meta.fs_scopes.clone(),
                manifest_path: toml_path
                    .strip_prefix(&self.root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| toml_path.display().to_string()),
            });
        }

        Ok(records)
    }

    fn enabled_set(&self) -> Option<HashSet<String>> {
        load_startup_config(&self.root).enabled_extensions
    }
}

#[derive(Clone)]
struct ExtensionRecord {
    id: String,
    version: String,
    kind: String,
    requires: Vec<String>,
    optional: Vec<String>,
    command_count: usize,
    fs_scopes: Vec<String>,
    manifest_path: String,
}

impl Extension for RegistryExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let extensions_dir = self.extensions_dir.clone();
        let root = self.root.clone();

        // extensions:list@1
        {
            let ext = self.clone_for_handler();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "extensions".into(),
                        action: "list".into(),
                        version: 1,
                    },
                    owner: "registry".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "extensions": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let records = ext.scan_manifests()?;
                    let profile = ext.enabled_set();
                    let prefs = crate::workbench_prefs::load(&ext.root);
                    let extensions: Vec<serde_json::Value> = records
                        .into_iter()
                        .map(|r| {
                            let in_profile = profile
                                .as_ref()
                                .map(|set| set.contains(&r.id))
                                .unwrap_or(true);
                            let runtime_enabled =
                                in_profile && prefs.extension_enabled(&r.id);
                            serde_json::json!({
                                "id": r.id,
                                "version": r.version,
                                "kind": r.kind,
                                "requires": r.requires,
                                "optional": r.optional,
                                "command_count": r.command_count,
                                "fs_scopes": r.fs_scopes,
                                "manifest_path": r.manifest_path,
                                "in_profile": in_profile,
                                "enabled": runtime_enabled,
                                "protected": crate::workbench_prefs::is_protected_extension(&r.id),
                            })
                        })
                        .collect();
                    Ok(serde_json::json!({ "extensions": extensions }))
                }),
            )?;
        }

        // extensions:graph@1
        {
            let ext = self.clone_for_handler();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "extensions".into(),
                        action: "graph".into(),
                        version: 1,
                    },
                    owner: "registry".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "nodes": "array", "edges": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let records = ext.scan_manifests()?;
                    let profile = ext.enabled_set();
                    let prefs = crate::workbench_prefs::load(&ext.root);
                    let id_set: HashSet<_> = records.iter().map(|r| r.id.clone()).collect();

                    let nodes: Vec<serde_json::Value> = records
                        .iter()
                        .map(|r| {
                            let in_profile = profile
                                .as_ref()
                                .map(|set| set.contains(&r.id))
                                .unwrap_or(true);
                            let runtime_enabled =
                                in_profile && prefs.extension_enabled(&r.id);
                            serde_json::json!({
                                "id": r.id,
                                "kind": r.kind,
                                "enabled": runtime_enabled,
                            })
                        })
                        .collect();

                    let mut edges = Vec::new();
                    for r in &records {
                        for dep in &r.requires {
                            if id_set.contains(dep) {
                                edges.push(serde_json::json!({
                                    "from": r.id,
                                    "to": dep,
                                    "kind": "requires",
                                }));
                            }
                        }
                        for dep in &r.optional {
                            if id_set.contains(dep) {
                                edges.push(serde_json::json!({
                                    "from": r.id,
                                    "to": dep,
                                    "kind": "optional",
                                }));
                            }
                        }
                    }

                    Ok(serde_json::json!({ "nodes": nodes, "edges": edges }))
                }),
            )?;
        }

        // commands:catalog@1
        {
            let cmds = ctx.commands.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "commands".into(),
                        action: "catalog".into(),
                        version: 1,
                    },
                    owner: "registry".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "commands": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let prefs = crate::workbench_prefs::load(&root);
                    let commands: Vec<serde_json::Value> = cmds
                        .command_catalog()
                        .into_iter()
                        .map(|mut entry| {
                            if let Some(owner) = entry.get("owner").and_then(|v| v.as_str()) {
                                let enabled = prefs.extension_enabled(owner);
                                if let Some(obj) = entry.as_object_mut() {
                                    obj.insert("enabled".into(), serde_json::json!(enabled));
                                }
                            }
                            entry
                        })
                        .collect();
                    Ok(serde_json::json!({ "commands": commands }))
                }),
            )?;
        }

        // workbench:prefs@1
        {
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "workbench".into(),
                        action: "prefs".into(),
                        version: 1,
                    },
                    owner: "registry".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "disabled_extensions": "array", "disabled_skills": "array", "disabled_rules": "array", "disabled_agents": "array" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let prefs = crate::workbench_prefs::load(&root);
                    Ok(serde_json::to_value(prefs)
                        .map_err(|e| CoreError::Io(format!("serialize prefs: {e}")))?)
                }),
            )?;
        }

        // workbench:set-enabled@1
        {
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "workbench".into(),
                        action: "set-enabled".into(),
                        version: 1,
                    },
                    owner: "registry".into(),
                    input_schema: r#"{ "kind": "string", "id": "string", "enabled": "boolean" }"#.into(),
                    output_schema: r#"{ "kind": "string", "id": "string", "enabled": "boolean" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let kind = input["kind"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("set-enabled: 'kind' required".into()))?;
                    let id = input["id"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("set-enabled: 'id' required".into()))?;
                    let enabled = input["enabled"]
                        .as_bool()
                        .ok_or_else(|| CoreError::Io("set-enabled: 'enabled' required".into()))?;
                    let mut prefs = crate::workbench_prefs::load(&root);
                    match kind {
                        "extension" => prefs.set_extension_enabled(id, enabled),
                        "skill" => prefs.set_skill_enabled(id, enabled),
                        "rule" => prefs.set_rule_enabled(id, enabled),
                        "agent" => prefs.set_agent_enabled(id, enabled),
                        other => {
                            return Err(CoreError::Io(format!(
                                "set-enabled: unknown kind '{other}'"
                            )))
                        }
                    }
                    crate::workbench_prefs::save(&root, &prefs)?;
                    Ok(serde_json::json!({ "kind": kind, "id": id, "enabled": enabled }))
                }),
            )?;
        }

        // workbench:reset@1
        {
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "workbench".into(),
                        action: "reset".into(),
                        version: 1,
                    },
                    owner: "registry".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "reset": "boolean" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |_| {
                    let mut prefs = crate::workbench_prefs::load(&root);
                    prefs.reset();
                    crate::workbench_prefs::save(&root, &prefs)?;
                    Ok(serde_json::json!({ "reset": true }))
                }),
            )?;
        }

        let _ = extensions_dir;
        let _ = root;
        eprintln!("[registry] activated");
        Ok(())
    }
}

impl RegistryExtension {
    fn clone_for_handler(&self) -> Self {
        Self {
            manifest: self.manifest.clone(),
            extensions_dir: self.extensions_dir.clone(),
            root: self.root.clone(),
        }
    }
}

fn resolve_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(parent) = cwd.parent() {
        if parent.join("extensions").exists() || parent.join("AGENTS.md").exists() {
            return parent.to_path_buf();
        }
    }
    cwd
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capability::{CapabilityRegistry, Capabilities},
        commands::CommandRegistry,
        context::InMemoryConfigStore,
        events::EventBus,
        permission::PermissionGate,
        runtime::Runtime,
        version::VersionManager,
    };
    use std::fs;

    fn make_context() -> CoreContext {
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

    fn make_manifest() -> ExtensionManifest {
        use crate::types::ExtensionKind;
        ExtensionManifest {
            id: "registry".into(),
            version: semver::Version::parse("0.1.0").unwrap(),
            kind: ExtensionKind::Service,
            api_version: "v1".into(),
            schema_version: semver::Version::parse("1.0.0").unwrap(),
            min_core: semver::Version::parse("0.1.0").unwrap(),
            requires: vec![],
            optional: vec![],
            provides: vec![],
            commands: vec![],
            publishes: vec![],
            subscribes: vec![],
            fs_scopes: vec![],
            http_hosts: vec![],
        }
    }

    #[test]
    fn registry_lists_extensions_on_disk() {
        let root = std::env::temp_dir().join(format!(
            "nulqor-registry-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(root.join("extensions/demo/src")).unwrap();
        fs::write(
            root.join("extensions/demo/extension.toml"),
            r#"[extension]
id = "demo"
version = "0.1.0"
kind = "Service"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
requires = ["host"]
"#,
        )
        .unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(&root).unwrap();
        let ctx = make_context();
        let ext = RegistryExtension::new(make_manifest());
        ext.activate(&ctx).unwrap();

        let list = ctx
            .commands
            .invoke(
                "test",
                &CommandId::parse("extensions:list@1").unwrap(),
                serde_json::json!({}),
            )
            .unwrap();
        std::env::set_current_dir(prev).unwrap();

        let extensions = list["extensions"].as_array().unwrap();
        assert!(extensions.iter().any(|e| e["id"] == "demo"));
    }
}
