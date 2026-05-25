//! Permission gate — enforces the four permission classes on every invocation (DESIGN.md §5).
//!
//! - read:       allowed without confirmation or logging.
//! - write:      allowed; action is logged.
//! - destructive: requires the confirm hook to return `true`; denied if no hook is set.
//! - system:     restricted to core-trusted callers; always denied from extensions in Phase 1.

use std::sync::{Arc, RwLock};

use crate::error::CoreError;
use crate::types::Permission;

type ConfirmHook = Arc<dyn Fn(&str) -> bool + Send + Sync>;

pub struct PermissionGate {
    /// Called for `destructive` actions. Returns `true` to allow.
    /// If `None`, all destructive actions are denied (safe default for Phase 1).
    confirm_hook: RwLock<Option<ConfirmHook>>,
}

impl PermissionGate {
    pub fn new() -> Self {
        Self { confirm_hook: RwLock::new(None) }
    }

    /// Install a confirm hook. Called for every `destructive` action.
    pub fn set_confirm_hook(&self, hook: ConfirmHook) {
        *self.confirm_hook.write().unwrap() = Some(hook);
    }

    /// Check whether `caller` may perform an action needing `needed` on `what`.
    pub fn check(&self, caller: &str, needed: Permission, what: &str) -> Result<(), CoreError> {
        match needed {
            Permission::Read => Ok(()),

            Permission::Write => {
                // Log every write — surfaced in Phase 4+ persistent logs.
                eprintln!("[WRITE] caller={caller} what={what}");
                Ok(())
            }

            Permission::Destructive => {
                let hook = self.confirm_hook.read().unwrap();
                match hook.as_ref() {
                    Some(f) if f(what) => Ok(()),
                    Some(_) => Err(CoreError::PermissionDenied {
                        caller: caller.to_owned(),
                        needed,
                        what: what.to_owned(),
                    }),
                    None => Err(CoreError::PermissionDenied {
                        caller: caller.to_owned(),
                        needed,
                        what: format!("{what} (no confirm hook installed — denied by default)"),
                    }),
                }
            }

            Permission::System => Err(CoreError::PermissionDenied {
                caller: caller.to_owned(),
                needed,
                what: what.to_owned(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_always_allowed() {
        let gate = PermissionGate::new();
        assert!(gate.check("ext", Permission::Read, "some-file").is_ok());
    }

    #[test]
    fn write_allowed_without_hook() {
        let gate = PermissionGate::new();
        assert!(gate.check("ext", Permission::Write, "some-state").is_ok());
    }

    #[test]
    fn destructive_denied_without_hook() {
        let gate = PermissionGate::new();
        assert!(gate.check("ext", Permission::Destructive, "delete-all").is_err());
    }

    #[test]
    fn destructive_allowed_when_hook_returns_true() {
        let gate = PermissionGate::new();
        gate.set_confirm_hook(Arc::new(|_| true));
        assert!(gate.check("ext", Permission::Destructive, "delete-all").is_ok());
    }

    #[test]
    fn destructive_denied_when_hook_returns_false() {
        let gate = PermissionGate::new();
        gate.set_confirm_hook(Arc::new(|_| false));
        assert!(gate.check("ext", Permission::Destructive, "delete-all").is_err());
    }

    #[test]
    fn system_always_denied_in_phase1() {
        let gate = PermissionGate::new();
        assert!(gate.check("ext", Permission::System, "spawn-sidecar").is_err());
    }
}
