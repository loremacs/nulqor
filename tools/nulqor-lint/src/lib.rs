//! Nulqor extension manifest linter.
//!
//! Enforces the rules from DESIGN.md §4, §5, §6, §7, §10, §12.
//! Outputs `FAIL: <path>: <reason>` lines. Never outputs prose.
//! Exit code 1 if any FAIL lines were emitted; 0 otherwise.

use serde::Deserialize;
use std::path::Path;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Lint a single extension directory and return `FAIL:` messages.
/// `ext_dir` must be the directory that contains `extension.toml`.
/// `root` is the repo root (used for depth calculations and relative paths).
/// Used by the in-app loader before activating an extension.
pub fn lint_extension_dir(ext_dir: &Path, root: &Path) -> Vec<String> {
    lint_one_extension(ext_dir, root)
}

/// Parse `extension.toml` from the given path and return the raw manifest.
/// Returns `Err(message)` on I/O or TOML parse errors.
pub fn parse_manifest(path: &Path) -> Result<Manifest, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    toml::from_str(&content)
        .map_err(|e| format!("TOML parse error in {}: {e}", path.display()))
}

/// Lint all extensions under `root/extensions/`.
/// Returns every `FAIL: <path>: <reason>` line found.
pub fn lint_extensions(root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let extensions_dir = root.join("extensions");

    if !extensions_dir.exists() {
        // Not an error — repo might not have extensions yet.
        return errors;
    }

    let entries = match std::fs::read_dir(&extensions_dir) {
        Ok(e) => e,
        Err(err) => {
            errors.push(format!(
                "FAIL: extensions/: cannot read directory: {err}"
            ));
            return errors;
        }
    };

    for entry in entries.flatten() {
        let ext_dir = entry.path();
        if !ext_dir.is_dir() {
            continue;
        }
        errors.extend(lint_one_extension(&ext_dir, root));
    }

    errors
}

// ---------------------------------------------------------------------------
// Per-extension lint
// ---------------------------------------------------------------------------

fn lint_one_extension(ext_dir: &Path, root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    let manifest_path = ext_dir.join("extension.toml");
    let rel_manifest = rel_str(&manifest_path, root);
    let rel_ext = rel_str(ext_dir, root);

    // Check 1 (DESIGN.md §5): extension.toml must be present
    if !manifest_path.exists() {
        errors.push(format!(
            "FAIL: {rel_ext}: extension.toml not found (required for every extension)"
        ));
        return errors;
    }

    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(err) => {
            errors.push(format!("FAIL: {rel_manifest}: cannot read file: {err}"));
            return errors;
        }
    };

    let manifest: Manifest = match toml::from_str(&content) {
        Ok(m) => m,
        Err(err) => {
            errors.push(format!("FAIL: {rel_manifest}: TOML parse error: {err}"));
            return errors;
        }
    };

    errors.extend(validate_manifest(&manifest, &rel_manifest, root, ext_dir));
    errors
}

// ---------------------------------------------------------------------------
// Manifest validation
// ---------------------------------------------------------------------------

fn validate_manifest(
    m: &Manifest,
    file: &str,
    root: &Path,
    ext_dir: &Path,
) -> Vec<String> {
    let mut errors = Vec::new();
    let ext = &m.extension;

    // ── [extension] required fields ─────────────────────────────────────────

    // id: non-empty kebab-case (DESIGN.md §9 naming conventions)
    if ext.id.is_empty() {
        errors.push(format!("FAIL: {file}: extension.id is empty"));
    } else if !is_kebab_case(&ext.id) {
        errors.push(format!(
            "FAIL: {file}: extension.id '{}' must be kebab-case \
             (lowercase letters, digits, hyphens; no spaces, colons, or @)",
            ext.id
        ));
    }

    // version: valid semver (DESIGN.md §4)
    if ext.version.is_empty() {
        errors.push(format!("FAIL: {file}: extension.version is empty"));
    } else if semver::Version::parse(&ext.version).is_err() {
        errors.push(format!(
            "FAIL: {file}: extension.version '{}' is not valid semver (e.g. 0.1.0)",
            ext.version
        ));
    }

    // kind: must be one of the declared extension kinds (DESIGN.md §13)
    const VALID_KINDS: &[&str] =
        &["Panel", "Host", "Service", "Provider", "Tool", "Theme", "Bake"];
    if ext.kind.is_empty() {
        errors.push(format!("FAIL: {file}: extension.kind is empty"));
    } else if !VALID_KINDS.contains(&ext.kind.as_str()) {
        errors.push(format!(
            "FAIL: {file}: extension.kind '{}' is not valid; must be one of: {}",
            ext.kind,
            VALID_KINDS.join(", ")
        ));
    }

    // api-version: required, must be "v<N>" (DESIGN.md §4)
    if ext.api_version.is_empty() {
        errors.push(format!("FAIL: {file}: extension.api-version is empty"));
    } else {
        let v = &ext.api_version;
        let ok = v.starts_with('v') && v[1..].parse::<u32>().is_ok();
        if !ok {
            errors.push(format!(
                "FAIL: {file}: extension.api-version '{}' must be 'v<N>' (e.g. v1)",
                v
            ));
        }
    }

    // schema-version: valid semver
    if ext.schema_version.is_empty() {
        errors.push(format!("FAIL: {file}: extension.schema-version is empty"));
    } else if semver::Version::parse(&ext.schema_version).is_err() {
        errors.push(format!(
            "FAIL: {file}: extension.schema-version '{}' is not valid semver",
            ext.schema_version
        ));
    }

    // min-core: valid semver
    if ext.min_core.is_empty() {
        errors.push(format!("FAIL: {file}: extension.min-core is empty"));
    } else if semver::Version::parse(&ext.min_core).is_err() {
        errors.push(format!(
            "FAIL: {file}: extension.min-core '{}' is not valid semver",
            ext.min_core
        ));
    }

    // ── [[commands]] ────────────────────────────────────────────────────────
    // id format: namespace:action@version (DESIGN.md §4, §5)
    for (i, cmd) in m.commands.iter().enumerate() {
        if let Err(e) = validate_versioned_id(&cmd.id) {
            errors.push(format!(
                "FAIL: {file}: commands[{i}].id '{}': {e}",
                cmd.id
            ));
        }
        if cmd.owner.is_empty() {
            errors.push(format!(
                "FAIL: {file}: commands[{i}].owner is empty (every command must declare an owner)"
            ));
        }
        const VALID_PERMS: &[&str] = &["read", "write", "destructive", "system"];
        if !cmd.permission.is_empty() && !VALID_PERMS.contains(&cmd.permission.as_str()) {
            errors.push(format!(
                "FAIL: {file}: commands[{i}].permission '{}' must be one of: {}",
                cmd.permission,
                VALID_PERMS.join(", ")
            ));
        }
    }

    // ── [[publishes]] ───────────────────────────────────────────────────────
    // id format: namespace:name@version (DESIGN.md §6)
    for (i, pub_decl) in m.publishes.iter().enumerate() {
        if let Err(e) = validate_versioned_id(&pub_decl.id) {
            errors.push(format!(
                "FAIL: {file}: publishes[{i}].id '{}': {e}",
                pub_decl.id
            ));
        }
    }

    // ── [[subscribes]] ──────────────────────────────────────────────────────
    // namespace required and must be kebab-case (DESIGN.md §6)
    for (i, sub) in m.subscribes.iter().enumerate() {
        if sub.namespace.is_empty() {
            errors.push(format!(
                "FAIL: {file}: subscribes[{i}].namespace is empty"
            ));
        } else if !is_kebab_case(&sub.namespace) {
            errors.push(format!(
                "FAIL: {file}: subscribes[{i}].namespace '{}' must be kebab-case",
                sub.namespace
            ));
        }
    }

    // ── [[provides]] ────────────────────────────────────────────────────────
    for (i, prov) in m.provides.iter().enumerate() {
        if prov.capability.is_empty() {
            errors.push(format!(
                "FAIL: {file}: provides[{i}].capability is empty"
            ));
        }
        if prov.instance.is_empty() {
            errors.push(format!(
                "FAIL: {file}: provides[{i}].instance is empty"
            ));
        }
        if let Err(e) = validate_contract_id(&prov.contract) {
            errors.push(format!(
                "FAIL: {file}: provides[{i}].contract '{}': {e}",
                prov.contract
            ));
        }
    }

    // ── fs-scopes boundary check ─────────────────────────────────────────────
    // DESIGN.md §7: extensions cannot reference another extension's files.
    for scope in &ext.fs_scopes {
        if is_cross_extension_ref(scope) {
            errors.push(format!(
                "FAIL: {file}: fs-scopes entry '{scope}' escapes the extension boundary \
                 (BOUNDARY=ERROR; cross-extension file access is forbidden — DESIGN.md §7)"
            ));
        }
    }

    // ── Directory depth check ────────────────────────────────────────────────
    // DESIGN.md §10: max depth 5 from repo root. Exception: skills/<n>/scripts/<f> = 6.
    errors.extend(check_depth(ext_dir, root));

    errors
}

// ---------------------------------------------------------------------------
// Directory depth check (DESIGN.md §10)
// ---------------------------------------------------------------------------

fn check_depth(ext_dir: &Path, root: &Path) -> Vec<String> {
    let mut errors = Vec::new();
    walk_depth(ext_dir, root, &mut errors);
    errors
}

fn walk_depth(dir: &Path, root: &Path, errors: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Count depth as number of path components from repo root
        let depth = rel.components().count();
        let rel_display = rel.to_string_lossy().replace('\\', "/");

        if depth > 5 {
            // Skills exception: skills/<name>/scripts/<file> is allowed at depth 6
            let is_skills_script_exception = {
                let parts: Vec<_> = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect();
                parts.len() == 6
                    && parts[0] == "skills"
                    && parts[3] == "scripts"
                    && !path.is_dir()
            };

            if depth > 6 {
                errors.push(format!(
                    "FAIL: {rel_display}: depth {depth} exceeds absolute maximum of 6 (DESIGN.md §10)"
                ));
            } else if !is_skills_script_exception {
                errors.push(format!(
                    "FAIL: {rel_display}: depth {depth} exceeds maximum of 5 \
                     (only skills/<name>/scripts/<file> may reach depth 6 — DESIGN.md §10)"
                ));
            }
        }

        if path.is_dir() && depth < 6 {
            walk_depth(&path, root, errors);
        }
    }
}

// ---------------------------------------------------------------------------
// Identifier validation helpers
// ---------------------------------------------------------------------------

/// Validate `namespace:name@N` format. N must be a positive integer >= 1.
/// DESIGN.md §4 / §6.
fn validate_versioned_id(id: &str) -> Result<(), String> {
    let at_pos = id
        .rfind('@')
        .ok_or_else(|| "missing @version suffix; expected 'namespace:name@N'".to_string())?;

    let prefix = &id[..at_pos];
    let version_str = &id[at_pos + 1..];

    let version: u32 = version_str.parse().map_err(|_| {
        format!(
            "version after '@' must be a positive integer, got '{version_str}'; \
             expected 'namespace:name@N' (e.g. hello:ping@1)"
        )
    })?;

    if version == 0 {
        return Err("version must be >= 1 (version 0 is reserved)".to_string());
    }

    let colon_count = prefix.chars().filter(|&c| c == ':').count();
    if colon_count != 1 {
        return Err(format!(
            "expected exactly one ':' in 'namespace:name', found {colon_count} in '{prefix}'"
        ));
    }

    let mut parts = prefix.splitn(2, ':');
    let namespace = parts.next().unwrap_or("");
    let name = parts.next().unwrap_or("");

    if !is_kebab_case(namespace) {
        return Err(format!(
            "namespace '{namespace}' must be non-empty kebab-case \
             (lowercase letters, digits, hyphens)"
        ));
    }
    if !is_kebab_case(name) {
        return Err(format!(
            "name '{name}' must be non-empty kebab-case \
             (lowercase letters, digits, hyphens)"
        ));
    }

    Ok(())
}

/// Validate `capability@N` format used in [[provides]].contract.
fn validate_contract_id(contract: &str) -> Result<(), String> {
    let at_pos = contract
        .rfind('@')
        .ok_or_else(|| "missing @version; expected 'capability@N' (e.g. storage@1)".to_string())?;

    let name = &contract[..at_pos];
    let version_str = &contract[at_pos + 1..];

    if name.is_empty() {
        return Err("capability name before '@' is empty".to_string());
    }
    let _: u32 = version_str.parse().map_err(|_| {
        format!("version after '@' must be a positive integer, got '{version_str}'")
    })?;

    Ok(())
}

/// Returns `true` if a path string appears to escape the extension directory.
/// DESIGN.md §7: cross-extension file access is a boundary violation.
fn is_cross_extension_ref(scope: &str) -> bool {
    let normalized = scope.replace('\\', "/");
    // Direct parent escapes
    if normalized.starts_with("../") || normalized == ".." {
        return true;
    }
    // Count ".." vs forward components to detect net-upward paths
    if normalized.contains("..") {
        let up: usize = normalized.split('/').filter(|s| *s == "..").count();
        let down: usize = normalized
            .split('/')
            .filter(|s| !s.is_empty() && *s != "..")
            .count();
        if up > down {
            return true;
        }
    }
    false
}

/// Returns `true` if `s` is valid kebab-case: non-empty, lowercase ASCII letters/digits/hyphens,
/// not starting or ending with a hyphen.
fn is_kebab_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// Format a path relative to `root` with forward slashes (for consistent FAIL messages).
fn rel_str(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// Deserialization structs for extension.toml
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct Manifest {
    pub extension: ExtensionMeta,
    #[serde(default)]
    pub provides: Vec<ProvideDecl>,
    #[serde(default)]
    pub commands: Vec<CommandDecl>,
    #[serde(default)]
    pub publishes: Vec<PublishDecl>,
    #[serde(default)]
    pub subscribes: Vec<SubscribeDecl>,
}

#[derive(Deserialize)]
pub struct ExtensionMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub kind: String,
    #[serde(rename = "api-version", default)]
    pub api_version: String,
    #[serde(rename = "schema-version", default)]
    pub schema_version: String,
    #[serde(rename = "min-core", default)]
    pub min_core: String,
    /// Declared filesystem scopes — checked for boundary violations (DESIGN.md §7).
    #[serde(rename = "fs-scopes", default)]
    pub fs_scopes: Vec<String>,
    /// Declared HTTP hosts — format validated in Phase 1+.
    #[serde(rename = "http-hosts", default)]
    #[allow(dead_code)]
    pub http_hosts: Vec<String>,
    /// Extension IDs this extension depends on (load order).
    #[serde(default)]
    pub requires: Vec<String>,
    /// Optional extension IDs (loaded if present, skipped if absent).
    #[serde(default)]
    pub optional: Vec<String>,
}

#[derive(Deserialize)]
pub struct CommandDecl {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub owner: String,
    /// Parsed but validation deferred to Phase 1+.
    #[allow(dead_code)]
    #[serde(rename = "input-schema", default)]
    pub input_schema: String,
    #[allow(dead_code)]
    #[serde(rename = "output-schema", default)]
    pub output_schema: String,
    #[allow(dead_code)]
    #[serde(rename = "callable-by", default)]
    pub callable_by: Vec<String>,
    #[serde(default)]
    pub permission: String,
}

#[derive(Deserialize)]
pub struct PublishDecl {
    #[serde(default)]
    pub id: String,
}

#[derive(Deserialize)]
pub struct SubscribeDecl {
    #[serde(default)]
    pub namespace: String,
    /// Optional event name filter — validated in Phase 1+.
    #[allow(dead_code)]
    pub name: Option<String>,
    #[allow(dead_code)]
    pub version: Option<String>,
}

#[derive(Deserialize)]
pub struct ProvideDecl {
    #[serde(default)]
    pub capability: String,
    #[serde(default)]
    pub instance: String,
    #[serde(default)]
    pub contract: String,
}
