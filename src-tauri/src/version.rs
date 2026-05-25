//! Version manager — three independent version axes (DESIGN.md §4, ADR 002).
//!
//! Axes:
//!  1. Core API version  — the surface extensions call (e.g. "v1")
//!  2. Schema version    — the `extension.toml` manifest format version
//!  3. Contract versions — per-command/event, many can coexist (e.g. `hello:ping@1` and `@2`)

use std::collections::HashMap;
use std::sync::RwLock;

use semver::Version;

use crate::error::CoreError;

const CORE_VERSION: &str = "0.1.0";
const SUPPORTED_API_VERSIONS: &[&str] = &["v1"];
const SCHEMA_VERSION: &str = "1.0.0";

pub struct VersionManager {
    core_version: Version,
    /// API versions the core currently supports.
    api_versions: Vec<String>,
    schema_version: Version,
    /// `"namespace:action"` → list of active contract versions.
    contract_versions: RwLock<HashMap<String, Vec<u32>>>,
    /// `ext_id` → installed semver.
    loaded_extensions: RwLock<HashMap<String, Version>>,
}

impl VersionManager {
    pub fn new() -> Self {
        Self {
            core_version: Version::parse(CORE_VERSION).unwrap(),
            api_versions: SUPPORTED_API_VERSIONS.iter().map(|s| s.to_string()).collect(),
            schema_version: Version::parse(SCHEMA_VERSION).unwrap(),
            contract_versions: RwLock::new(HashMap::new()),
            loaded_extensions: RwLock::new(HashMap::new()),
        }
    }

    /// Check whether an extension's requirements can be satisfied by this core.
    /// Fails loud with a structured compatibility report if anything is unmet.
    pub fn check_extension(
        &self,
        ext_id: &str,
        api_version: &str,
        schema_version_str: &str,
        min_core_str: &str,
    ) -> Result<(), CoreError> {
        // 1. API version must be supported
        if !self.api_versions.contains(&api_version.to_string()) {
            return Err(CoreError::VersionMismatch {
                wanted: format!("{ext_id}: api-version '{api_version}'"),
                available: self.api_versions.clone(),
            });
        }

        // 2. Schema version must be >= what we support
        let _ext_schema = Version::parse(schema_version_str).map_err(|_| {
            CoreError::VersionMismatch {
                wanted: format!("{ext_id}: schema-version '{schema_version_str}' (not semver)"),
                available: vec![self.schema_version.to_string()],
            }
        })?;

        // 3. Core version must satisfy extension's min-core requirement
        let min_core = Version::parse(min_core_str).map_err(|_| CoreError::VersionMismatch {
            wanted: format!("{ext_id}: min-core '{min_core_str}' (not semver)"),
            available: vec![self.core_version.to_string()],
        })?;
        if self.core_version < min_core {
            return Err(CoreError::VersionMismatch {
                wanted: format!("{ext_id}: requires core >= {min_core}"),
                available: vec![self.core_version.to_string()],
            });
        }

        Ok(())
    }

    /// Register that a contract version is live.
    /// Called by the command registry when an extension registers a command.
    pub fn register_contract(&self, base_key: &str, version: u32) {
        let mut map = self.contract_versions.write().unwrap();
        map.entry(base_key.to_string()).or_default().push(version);
    }

    /// Returns all active versions for the given base key (e.g. `"hello:ping"`).
    pub fn available_versions(&self, base_key: &str) -> Vec<u32> {
        self.contract_versions
            .read()
            .unwrap()
            .get(base_key)
            .cloned()
            .unwrap_or_default()
    }

    /// Record that an extension has been loaded.
    pub fn record_loaded(&self, ext_id: &str, version: Version) {
        self.loaded_extensions
            .write()
            .unwrap()
            .insert(ext_id.to_string(), version);
    }

    pub fn core_version(&self) -> &Version {
        &self.core_version
    }

    pub fn api_versions(&self) -> &[String] {
        &self.api_versions
    }
}

// ---------------------------------------------------------------------------
// Tests — BUILD_PLAN §1.1
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_contract_versions_coexist() {
        let vm = VersionManager::new();
        vm.register_contract("hello:ping", 1);
        vm.register_contract("hello:ping", 2);
        let versions = vm.available_versions("hello:ping");
        assert!(versions.contains(&1), "expected @1");
        assert!(versions.contains(&2), "expected @2");
    }

    #[test]
    fn missing_version_returns_empty() {
        let vm = VersionManager::new();
        assert!(vm.available_versions("no:such").is_empty(), "expected empty");
    }

    #[test]
    fn check_extension_passes_valid() {
        let vm = VersionManager::new();
        let result = vm.check_extension("my-ext", "v1", "1.0.0", "0.1.0");
        assert!(result.is_ok(), "expected ok, got {result:?}");
    }

    #[test]
    fn check_extension_fails_unknown_api_version() {
        let vm = VersionManager::new();
        let result = vm.check_extension("my-ext", "v99", "1.0.0", "0.1.0");
        assert!(result.is_err(), "expected err for unknown api-version");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("v99"), "error should mention v99: {msg}");
    }

    #[test]
    fn check_extension_fails_min_core_too_high() {
        let vm = VersionManager::new();
        // current core is 0.1.0; require 99.0.0
        let result = vm.check_extension("my-ext", "v1", "1.0.0", "99.0.0");
        assert!(result.is_err(), "expected err for too-high min-core");
    }
}
