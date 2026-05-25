//! Skill-runner extension — Phase 3.1 (BUILD_PLAN §3.1).
//!
//! Provides on-demand skill loading and injection. Wraps `context-editor:load-skill@1` with
//! execution logging so every skill use is traceable.
//!
//! Commands:
//!   - `skill-runner:load@1`  — load a named skill, log the invocation.
//!   - `skill-runner:list@1`  — list available skills (delegates to context-editor).

use std::sync::Arc;

use crate::commands::CommandRegistry;
use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

pub struct SkillRunnerExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl SkillRunnerExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for SkillRunnerExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let cmds = ctx.commands.clone();
        register_load(cmds.clone(), &ctx.commands)?;
        register_list(cmds, &ctx.commands)?;
        eprintln!("[skill-runner] activated");
        Ok(())
    }
}

fn register_load(
    cmds: Arc<CommandRegistry>,
    registry: &Arc<CommandRegistry>,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "skill-runner".into(),
                action: "load".into(),
                version: 1,
            },
            owner: "skill-runner".into(),
            input_schema: r#"{ "name": "string" }"#.into(),
            output_schema: r#"{ "name": "string", "body": "string", "found": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let name = input["name"]
                .as_str()
                .ok_or_else(|| CoreError::Io("skill-runner:load requires 'name'".into()))?
                .to_owned();

            let load_id = CommandId::parse("context-editor:load-skill@1")
                .map_err(|e| CoreError::Io(e.to_string()))?;

            match cmds.invoke("skill-runner", &load_id, serde_json::json!({ "name": &name })) {
                Ok(result) => {
                    eprintln!("[skill-runner] loaded skill: {name}");
                    Ok(serde_json::json!({
                        "name": &name,
                        "body": result["body"],
                        "found": true,
                    }))
                }
                Err(_) => {
                    eprintln!("[skill-runner] skill not found: {name}");
                    Ok(serde_json::json!({ "name": &name, "body": "", "found": false }))
                }
            }
        }),
    )
}

fn register_list(
    cmds: Arc<CommandRegistry>,
    registry: &Arc<CommandRegistry>,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "skill-runner".into(),
                action: "list".into(),
                version: 1,
            },
            owner: "skill-runner".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "skills": "array" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let list_id = CommandId::parse("context-editor:list-skills@1")
                .map_err(|e| CoreError::Io(e.to_string()))?;
            cmds.invoke("skill-runner", &list_id, serde_json::json!({}))
        }),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::InMemoryConfigStore;
    use crate::types::ExtensionKind;
    use std::sync::Arc;

    fn make_context() -> CoreContext {
        use crate::{
            capability::{CapabilityRegistry, Capabilities},
            commands::CommandRegistry,
            events::EventBus,
            permission::PermissionGate,
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
            runtime: Arc::new(crate::runtime::Runtime::new()),
            config: Arc::new(InMemoryConfigStore::new()),
        }
    }

    fn make_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: "skill-runner".into(),
            version: semver::Version::parse("0.1.0").unwrap(),
            kind: ExtensionKind::Service,
            api_version: "v1".into(),
            schema_version: semver::Version::parse("1.0.0").unwrap(),
            min_core: semver::Version::parse("0.1.0").unwrap(),
            requires: vec!["context-editor".into()],
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
    fn skill_runner_registers_commands() {
        let ctx = make_context();

        // stub the context-editor dependency
        let stub_id = CommandId { namespace: "context-editor".into(), action: "load-skill".into(), version: 1 };
        ctx.commands.register(
            CommandDecl {
                id: stub_id,
                owner: "context-editor".into(),
                input_schema: "{}".into(),
                output_schema: "{}".into(),
                callable_by: vec!["service".into()],
                permission: Permission::Read,
            },
            Arc::new(|_| Ok(serde_json::json!({ "name": "x", "description": "", "body": "body" }))),
        ).unwrap();

        let stub_list_id = CommandId { namespace: "context-editor".into(), action: "list-skills".into(), version: 1 };
        ctx.commands.register(
            CommandDecl {
                id: stub_list_id,
                owner: "context-editor".into(),
                input_schema: "{}".into(),
                output_schema: "{}".into(),
                callable_by: vec!["service".into()],
                permission: Permission::Read,
            },
            Arc::new(|_| Ok(serde_json::json!({ "skills": [] }))),
        ).unwrap();

        let ext = SkillRunnerExtension::new(make_manifest());
        ext.activate(&ctx).expect("activate");

        let cmds = ctx.commands.list_commands();
        assert!(cmds.iter().any(|c| c == "skill-runner:load@1"), "load command missing");
        assert!(cmds.iter().any(|c| c == "skill-runner:list@1"), "list command missing");
    }

    #[test]
    fn load_missing_skill_returns_not_found() {
        let ctx = make_context();

        // stub: skill not found
        let stub_id = CommandId { namespace: "context-editor".into(), action: "load-skill".into(), version: 1 };
        ctx.commands.register(
            CommandDecl {
                id: stub_id,
                owner: "context-editor".into(),
                input_schema: "{}".into(),
                output_schema: "{}".into(),
                callable_by: vec!["service".into()],
                permission: Permission::Read,
            },
            Arc::new(|_| Err(CoreError::Io("skill not found".into()))),
        ).unwrap();

        let stub_list_id = CommandId { namespace: "context-editor".into(), action: "list-skills".into(), version: 1 };
        ctx.commands.register(
            CommandDecl {
                id: stub_list_id,
                owner: "context-editor".into(),
                input_schema: "{}".into(),
                output_schema: "{}".into(),
                callable_by: vec!["service".into()],
                permission: Permission::Read,
            },
            Arc::new(|_| Ok(serde_json::json!({ "skills": [] }))),
        ).unwrap();

        let ext = SkillRunnerExtension::new(make_manifest());
        ext.activate(&ctx).unwrap();

        let result = ctx.commands.invoke(
            "test",
            &CommandId::parse("skill-runner:load@1").unwrap(),
            serde_json::json!({ "name": "missing-skill" }),
        ).unwrap();

        assert_eq!(result["found"], false);
        assert_eq!(result["body"], "");
    }
}
