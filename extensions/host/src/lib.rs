//! Host extension — transparent canvas shell, grid desk, and menu bar UI.
//! BUILD_PLAN §1.9

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

pub struct HostExtension {
    manifest: ExtensionManifest,
}

impl HostExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for HostExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        ctx.commands.register(
            CommandDecl {
                id: CommandId {
                    namespace: "canvas".into(),
                    action: "status".into(),
                    version: 1,
                },
                owner: "host".into(),
                input_schema: "{}".into(),
                output_schema: r#"{ "ready": "boolean" }"#.into(),
                callable_by: vec!["panel".into(), "agent".into()],
                permission: Permission::Read,
            },
            Arc::new(|_input| Ok(serde_json::json!({ "ready": true }))),
        )?;

        let config = ctx.config.clone();
        ctx.commands.register(
            CommandDecl {
                id: CommandId {
                    namespace: "canvas".into(),
                    action: "config".into(),
                    version: 1,
                },
                owner: "host".into(),
                input_schema: "{}".into(),
                output_schema: r#"{ "open_panels": "array", "shell": "object", "panels": "array" }"#.into(),
                callable_by: vec!["panel".into(), "agent".into()],
                permission: Permission::Read,
            },
            Arc::new(move |_input| config.get("host", "canvas")),
        )?;

        Ok(())
    }
}
