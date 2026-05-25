//! Host extension — mounts the canvas window shell and emits `canvas:ready@1`.
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
        // Register the canvas:status@1 command so the frontend can query canvas state.
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

        // NOTE: canvas:ready@1 is emitted by lib.rs AFTER all extensions have
        // been loaded so late-registering subscribers (e.g. hello-panel) can receive it.

        Ok(())
    }
}
