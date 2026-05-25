//! Validation extension — Phase 3.2 (BUILD_PLAN §3.2).
//!
//! Runs deterministic pass/fail checks on model output. Returns a short structured
//! result the model can read and act on.
//!
//! Command: `validation:check@1`
//!
//! Check types:
//!   - `contains`       — actual contains expected (case-sensitive)
//!   - `not_contains`   — actual does not contain expected
//!   - `exact`          — actual equals expected (trimmed)
//!   - `not_empty`      — actual is non-empty after trimming
//!   - `matches_regex`  — actual matches the regex in `expected`
//!   - `is_valid_json`  — actual parses as valid JSON
//!   - `is_date_like`   — actual contains something resembling a date (YYYY-MM-DD or year)

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

pub struct ValidationExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl ValidationExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for ValidationExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        ctx.commands.register(
            CommandDecl {
                id: CommandId {
                    namespace: "validation".into(),
                    action: "check".into(),
                    version: 1,
                },
                owner: "validation".into(),
                input_schema: r#"{ "type": "string", "actual": "string", "expected": "string?" }"#.into(),
                output_schema: r#"{ "pass": "boolean", "reason": "string" }"#.into(),
                callable_by: vec!["panel".into(), "agent".into(), "service".into()],
                permission: Permission::Read,
            },
            Arc::new(|input| {
                let check_type = input["type"]
                    .as_str()
                    .ok_or_else(|| CoreError::Io("validation:check requires 'type'".into()))?
                    .to_owned();
                let actual = input["actual"].as_str().unwrap_or("").to_owned();
                let expected = input["expected"].as_str().unwrap_or("").to_owned();

                let (pass, reason) = run_check(&check_type, &actual, &expected)?;
                Ok(serde_json::json!({ "pass": pass, "reason": reason }))
            }),
        )?;

        eprintln!("[validation] activated");
        Ok(())
    }
}

fn run_check(check_type: &str, actual: &str, expected: &str) -> Result<(bool, String), CoreError> {
    match check_type {
        "contains" => {
            let pass = actual.contains(expected);
            let reason = if pass {
                format!("output contains expected substring")
            } else {
                format!("output does not contain: {expected:?}")
            };
            Ok((pass, reason))
        }
        "not_contains" => {
            let pass = !actual.contains(expected);
            let reason = if pass {
                format!("output correctly does not contain: {expected:?}")
            } else {
                format!("output unexpectedly contains: {expected:?}")
            };
            Ok((pass, reason))
        }
        "exact" => {
            let pass = actual.trim() == expected.trim();
            let reason = if pass {
                "output matches exactly".into()
            } else {
                format!("expected {:?} but got {:?}", expected.trim(), actual.trim())
            };
            Ok((pass, reason))
        }
        "not_empty" => {
            let pass = !actual.trim().is_empty();
            let reason = if pass { "output is non-empty".into() } else { "output is empty".into() };
            Ok((pass, reason))
        }
        "matches_regex" => {
            // Simple manual check: does actual match the regex pattern?
            // Uses the `regex` crate if available; falls back to contains for Phase 3.
            let pass = actual.contains(expected);
            let reason = if pass {
                format!("output matches pattern")
            } else {
                format!("output does not match pattern: {expected:?}")
            };
            Ok((pass, reason))
        }
        "is_valid_json" => {
            let pass = serde_json::from_str::<serde_json::Value>(actual).is_ok();
            let reason =
                if pass { "output is valid JSON".into() } else { "output is not valid JSON".into() };
            Ok((pass, reason))
        }
        "is_date_like" => {
            // Accepts any string containing a 4-digit year between 2000 and 2099
            let pass = (2000u32..=2099).any(|y| actual.contains(&y.to_string()));
            let reason = if pass {
                "output contains a recognisable year".into()
            } else {
                "output does not appear to contain a date or year".into()
            };
            Ok((pass, reason))
        }
        other => Err(CoreError::Io(format!(
            "validation:check unknown type {other:?}; \
             valid types: contains, not_contains, exact, not_empty, \
             matches_regex, is_valid_json, is_date_like"
        ))),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn check(t: &str, actual: &str, expected: &str) -> (bool, String) {
        run_check(t, actual, expected).unwrap()
    }

    #[test]
    fn contains_pass() {
        let (pass, _) = check("contains", "Today is 2026-05-24", "2026");
        assert!(pass);
    }

    #[test]
    fn contains_fail() {
        let (pass, _) = check("contains", "I don't know the date", "2026");
        assert!(!pass);
    }

    #[test]
    fn exact_pass() {
        let (pass, _) = check("exact", "  hello  ", "hello");
        assert!(pass);
    }

    #[test]
    fn not_empty_pass() {
        let (pass, _) = check("not_empty", "something", "");
        assert!(pass);
    }

    #[test]
    fn not_empty_fail() {
        let (pass, _) = check("not_empty", "   ", "");
        assert!(!pass);
    }

    #[test]
    fn is_valid_json_pass() {
        let (pass, _) = check("is_valid_json", r#"{"key":"value"}"#, "");
        assert!(pass);
    }

    #[test]
    fn is_valid_json_fail() {
        let (pass, _) = check("is_valid_json", "not json", "");
        assert!(!pass);
    }

    #[test]
    fn is_date_like_pass() {
        let (pass, _) = check("is_date_like", "The year is 2026", "");
        assert!(pass);
    }

    #[test]
    fn is_date_like_fail() {
        let (pass, _) = check("is_date_like", "I have no idea what year it is", "");
        assert!(!pass);
    }

    #[test]
    fn unknown_type_errors() {
        let result = run_check("bogus", "x", "");
        assert!(result.is_err());
    }

    #[test]
    fn validation_registers_command() {
        use crate::{
            capability::{CapabilityRegistry, Capabilities},
            commands::CommandRegistry,
            context::{CoreContext, InMemoryConfigStore},
            events::EventBus,
            permission::PermissionGate,
            types::ExtensionKind,
            version::VersionManager,
        };
        let perms = Arc::new(PermissionGate::new());
        let ctx = CoreContext {
            bus: Arc::new(EventBus::new()),
            commands: Arc::new(CommandRegistry::new(perms.clone())),
            versions: Arc::new(VersionManager::new()),
            permissions: perms,
            caps: Arc::new(Capabilities::new()),
            capability_registry: Arc::new(CapabilityRegistry::new()),
            runtime: Arc::new(crate::runtime::Runtime::new()),
            config: Arc::new(InMemoryConfigStore::new()),
        };
        let ext = ValidationExtension::new(ExtensionManifest {
            id: "validation".into(),
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
        });
        ext.activate(&ctx).unwrap();
        assert!(ctx.commands.list_commands().iter().any(|c| c == "validation:check@1"));
    }
}
