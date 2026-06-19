//! Capability layer — the ONLY door to filesystem, network, and processes (DESIGN.md §7, ADR 004).
//!
//! Extensions may NOT touch arbitrary files, hosts, or processes.
//! Every external access goes through this layer, which checks declared scopes/hosts.
//!
//! Phase 1 status:
//! - fs_read / fs_write: scope checking implemented; actual I/O stubbed (returns errors).
//! - http_request: host checking implemented; actual HTTP stubbed.
//! - spawn_sidecar: gated behind `system` permission; stubbed.
//! - Named instance registry: duplicate-instance detection implemented.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::CoreError;
use crate::types::{CapabilityDecl, HttpRequest, HttpResponse, SidecarHandle, SidecarSpec};

// ---------------------------------------------------------------------------
// Named capability / instance registry (DESIGN.md §5 seam-2 fix)
// ---------------------------------------------------------------------------

/// Registered capability provider handles. Key: `"capability/instance"`.
#[allow(dead_code)]
pub struct CapabilityRegistry {
    entries: RwLock<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self { entries: RwLock::new(HashMap::new()) }
    }

    /// Register a named capability instance. Fails loud on duplicate.
    pub fn provide(
        &self,
        ext_id: &str,
        decl: CapabilityDecl,
        handle: Arc<dyn std::any::Any + Send + Sync>,
    ) -> Result<(), CoreError> {
        let key = format!("{}/{}", decl.capability, decl.instance);
        let mut map = self.entries.write().map_err(|_| CoreError::Io("capability lock poisoned".into()))?;
        if map.contains_key(&key) {
            return Err(CoreError::DuplicateCapability {
                capability: decl.capability.clone(),
                instance: format!("{} (registered by '{ext_id}')", decl.instance),
            });
        }
        map.insert(key, handle);
        Ok(())
    }

    /// Resolve a named capability instance. Fails loud if not registered.
    pub fn resolve(
        &self,
        capability: &str,
        instance: &str,
    ) -> Result<Arc<dyn std::any::Any + Send + Sync>, CoreError> {
        let key = format!("{capability}/{instance}");
        self.entries
            .read()
            .map_err(|_| CoreError::Io("capability lock poisoned".into()))?
            .get(&key)
            .cloned()
            .ok_or_else(|| CoreError::UnknownCapability {
                kind: capability.to_owned(),
                instance: instance.to_owned(),
            })
    }
}

// ---------------------------------------------------------------------------
// Capabilities — scoped fs / http / sidecar access
// ---------------------------------------------------------------------------

/// Declared scopes loaded from extension manifests during activation.
#[allow(dead_code)]
struct ExtensionScopes {
    fs_scopes: Vec<String>,
    http_hosts: Vec<String>,
}

pub struct Capabilities {
    scopes: RwLock<HashMap<String, ExtensionScopes>>,
}

impl Capabilities {
    pub fn new() -> Self {
        Self { scopes: RwLock::new(HashMap::new()) }
    }

    /// Register declared scopes for an extension. Called by the loader during activation.
    pub fn register_scopes(
        &self,
        ext_id: &str,
        fs_scopes: Vec<String>,
        http_hosts: Vec<String>,
    ) {
        self.scopes.write().unwrap_or_else(|p| p.into_inner()).insert(
            ext_id.to_owned(),
            ExtensionScopes { fs_scopes, http_hosts },
        );
    }

    pub fn fs_read(&self, ext_id: &str, path: &str) -> Result<Vec<u8>, CoreError> {
        self.check_fs_scope(ext_id, path)?;
        // Phase 1 stub — actual I/O in Phase 4+
        Err(CoreError::Io(format!(
            "fs_read not yet implemented for '{path}' (Phase 1 stub)"
        )))
    }

    pub fn fs_write(&self, ext_id: &str, path: &str, _bytes: &[u8]) -> Result<(), CoreError> {
        self.check_fs_scope(ext_id, path)?;
        Err(CoreError::Io(format!(
            "fs_write not yet implemented for '{path}' (Phase 1 stub)"
        )))
    }

    pub fn http_request(
        &self,
        ext_id: &str,
        req: HttpRequest,
    ) -> Result<HttpResponse, CoreError> {
        self.check_http_host(ext_id, &req.url)?;
        Err(CoreError::Io(format!(
            "http_request not yet implemented for '{}' (Phase 1 stub)",
            req.url
        )))
    }

    pub fn spawn_sidecar(
        &self,
        _ext_id: &str,
        _spec: SidecarSpec,
    ) -> Result<SidecarHandle, CoreError> {
        // System-permission gate is enforced upstream by PermissionGate.
        Err(CoreError::Io("spawn_sidecar not yet implemented (Phase 1 stub)".into()))
    }

    // ── Scope checks ──────────────────────────────────────────────────────

    fn check_fs_scope(&self, ext_id: &str, path: &str) -> Result<(), CoreError> {
        let scopes = self.scopes.read().map_err(|_| CoreError::Io("capability lock poisoned".into()))?;
        if let Some(ext) = scopes.get(ext_id) {
            if ext.fs_scopes.is_empty() {
                return Err(CoreError::BoundaryViolation(format!(
                    "extension '{ext_id}' has no declared fs-scopes; access to '{path}' denied"
                )));
            }
            let allowed = ext.fs_scopes.iter().any(|scope| path.starts_with(scope.as_str()));
            if !allowed {
                return Err(CoreError::BoundaryViolation(format!(
                    "extension '{ext_id}' may not access '{path}' (not in declared fs-scopes)"
                )));
            }
        } else {
            return Err(CoreError::BoundaryViolation(format!(
                "extension '{ext_id}' has no registered scopes; fs access to '{path}' denied"
            )));
        }
        Ok(())
    }

    /// Public boundary check: returns `Ok` if `url` is covered by the extension's declared
    /// `http-hosts`. Call this before making external HTTP requests inside an extension handler.
    pub fn check_http_allowed(&self, ext_id: &str, url: &str) -> Result<(), CoreError> {
        self.check_http_host(ext_id, url)
    }

    fn check_http_host(&self, ext_id: &str, url: &str) -> Result<(), CoreError> {
        let scopes = self.scopes.read().map_err(|_| CoreError::Io("capability lock poisoned".into()))?;
        if let Some(ext) = scopes.get(ext_id) {
            let allowed = ext.http_hosts.iter().any(|h| url.contains(h.as_str()));
            if !allowed {
                return Err(CoreError::BoundaryViolation(format!(
                    "extension '{ext_id}' may not access '{url}' (not in declared http-hosts)"
                )));
            }
        } else {
            return Err(CoreError::BoundaryViolation(format!(
                "extension '{ext_id}' has no registered scopes; http access to '{url}' denied"
            )));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_registry_duplicate_fails() {
        let reg = CapabilityRegistry::new();
        let decl = CapabilityDecl {
            capability: "storage".into(),
            instance: "main".into(),
            contract: "storage@1".into(),
        };
        let handle: Arc<dyn std::any::Any + Send + Sync> = Arc::new(42u32);
        reg.provide("ext-a", decl.clone(), handle.clone()).unwrap();
        let dup = reg.provide("ext-b", decl, handle);
        assert!(dup.is_err(), "duplicate (capability, instance) must fail");
    }

    #[test]
    fn capability_registry_resolve_missing_fails_loud() {
        let reg = CapabilityRegistry::new();
        let result = reg.resolve("storage", "main");
        assert!(matches!(result, Err(CoreError::UnknownCapability { .. })));
    }

    #[test]
    fn fs_read_denied_when_not_in_scope() {
        let caps = Capabilities::new();
        caps.register_scopes("ext-a", vec!["/allowed".into()], vec![]);
        let result = caps.fs_read("ext-a", "/other/path");
        assert!(matches!(result, Err(CoreError::BoundaryViolation(_))));
    }

    #[test]
    fn fs_read_denied_when_no_scopes_declared() {
        let caps = Capabilities::new();
        caps.register_scopes("ext-a", vec![], vec![]);
        let result = caps.fs_read("ext-a", "/any/path");
        assert!(matches!(result, Err(CoreError::BoundaryViolation(_))));
    }
}
