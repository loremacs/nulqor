//! Versioned command registry — register/invoke by `namespace:action@version` (DESIGN.md §5).
//!
//! - Each command has exactly one owner, one permission class, and one version.
//! - Multiple versions of the same command can coexist (`@1` and `@2` at the same time).
//! - Invoking a missing version fails loud with `VersionMismatch` — never a silent fallback.
//! - Permission is checked via the `PermissionGate` before the handler runs.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::error::CoreError;
use crate::permission::PermissionGate;
use crate::types::{CommandDecl, CommandId};

type Handler =
    Arc<dyn Fn(serde_json::Value) -> Result<serde_json::Value, CoreError> + Send + Sync>;

struct Entry {
    decl: CommandDecl,
    handler: Handler,
}

pub struct CommandRegistry {
    handlers: RwLock<HashMap<String, Entry>>,
    gate: Arc<PermissionGate>,
}

impl CommandRegistry {
    pub fn new(gate: Arc<PermissionGate>) -> Self {
        Self { handlers: RwLock::new(HashMap::new()), gate }
    }

    /// Register a command. Returns `Err` if the same `id@version` is already registered.
    pub fn register(&self, decl: CommandDecl, handler: Handler) -> Result<(), CoreError> {
        let key = decl.id.key();
        let mut map = self.handlers.write().unwrap();
        if map.contains_key(&key) {
            return Err(CoreError::DuplicateCapability {
                capability: "command".into(),
                instance: key,
            });
        }
        map.insert(key, Entry { decl, handler });
        Ok(())
    }

    /// Invoke an exact command version.
    /// Fails with `UnknownCommand` (version mismatch) if the version is not registered.
    /// Permission is checked before the handler runs.
    pub fn invoke(
        &self,
        caller: &str,
        id: &CommandId,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, CoreError> {
        let key = id.key();
        let map = self.handlers.read().unwrap();
        let entry = map.get(&key).ok_or_else(|| {
            // Surface available versions in the error for diagnostics
            let base = id.base_key();
            let available: Vec<String> = map
                .keys()
                .filter(|k| k.starts_with(&base))
                .cloned()
                .collect();
            if available.is_empty() {
                CoreError::UnknownCommand(key.clone())
            } else {
                CoreError::VersionMismatch { wanted: key.clone(), available }
            }
        })?;

        self.gate.check(caller, entry.decl.permission, &key)?;
        (entry.handler)(input)
    }

    /// List all registered command ids (for diagnostics / IPC introspection).
    pub fn list_commands(&self) -> Vec<String> {
        self.handlers.read().unwrap().keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permission::PermissionGate;
    use crate::types::Permission;

    fn make_registry() -> CommandRegistry {
        CommandRegistry::new(Arc::new(PermissionGate::new()))
    }

    fn ping_handler() -> Handler {
        Arc::new(|_| Ok(serde_json::json!({ "pong": true })))
    }

    fn decl(ns: &str, action: &str, version: u32, perm: Permission) -> CommandDecl {
        CommandDecl {
            id: CommandId { namespace: ns.into(), action: action.into(), version },
            owner: ns.into(),
            input_schema: "{}".into(),
            output_schema: "{}".into(),
            callable_by: vec!["panel".into()],
            permission: perm,
        }
    }

    #[test]
    fn register_and_invoke() {
        let reg = make_registry();
        reg.register(decl("hello", "ping", 1, Permission::Read), ping_handler()).unwrap();
        let result = reg
            .invoke("test", &CommandId::parse("hello:ping@1").unwrap(), serde_json::json!({}))
            .unwrap();
        assert_eq!(result["pong"], true);
    }

    #[test]
    fn two_versions_coexist() {
        let reg = make_registry();
        reg.register(decl("hello", "ping", 1, Permission::Read), ping_handler()).unwrap();
        reg.register(
            decl("hello", "ping", 2, Permission::Read),
            Arc::new(|_| Ok(serde_json::json!({ "pong": 2 }))),
        )
        .unwrap();
        assert_eq!(
            reg.invoke("t", &CommandId::parse("hello:ping@1").unwrap(), serde_json::json!({}))
                .unwrap()["pong"],
            true
        );
        assert_eq!(
            reg.invoke("t", &CommandId::parse("hello:ping@2").unwrap(), serde_json::json!({}))
                .unwrap()["pong"],
            2
        );
    }

    #[test]
    fn invoke_missing_version_fails_loud() {
        let reg = make_registry();
        reg.register(decl("hello", "ping", 1, Permission::Read), ping_handler()).unwrap();
        // Ask for @99 — only @1 exists → should fail with VersionMismatch
        let result =
            reg.invoke("t", &CommandId::parse("hello:ping@99").unwrap(), serde_json::json!({}));
        assert!(result.is_err(), "expected error for missing version");
        match result.unwrap_err() {
            CoreError::VersionMismatch { wanted, available } => {
                assert!(wanted.contains("@99"), "error should mention @99");
                assert!(!available.is_empty(), "should list available versions");
            }
            other => panic!("expected VersionMismatch, got {other:?}"),
        }
    }

    #[test]
    fn invoke_completely_unknown_command_fails_loud() {
        let reg = make_registry();
        let result =
            reg.invoke("t", &CommandId::parse("no:such@1").unwrap(), serde_json::json!({}));
        assert!(matches!(result, Err(CoreError::UnknownCommand(_))));
    }

    #[test]
    fn duplicate_registration_fails() {
        let reg = make_registry();
        reg.register(decl("hello", "ping", 1, Permission::Read), ping_handler()).unwrap();
        let dup = reg.register(decl("hello", "ping", 1, Permission::Read), ping_handler());
        assert!(dup.is_err(), "duplicate registration must fail");
    }
}
