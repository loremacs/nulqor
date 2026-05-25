//! Hello-panel extension — the Phase 1 end-to-end proof.
//! Proves: loader → command registration → IPC invoke → event subscription.
//! BUILD_PLAN §1.10

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{EventPattern, ExtensionManifest};

pub struct HelloPanelExtension {
    manifest: ExtensionManifest,
}

impl HelloPanelExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for HelloPanelExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        // Register hello:ping@1 — the command the TypeScript frontend calls to prove IPC.
        ctx.register_read_command("hello", "ping", 1, "hello-panel", |_input| {
            Ok(serde_json::json!({ "pong": true, "source": "hello-panel" }))
        })?;

        // Subscribe to canvas:ready@1 so this panel mounts after the host is ready.
        ctx.bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(|_ev| {
                eprintln!("[hello-panel] received canvas:ready@1 — panel would mount here");
            }),
        );

        Ok(())
    }
}
