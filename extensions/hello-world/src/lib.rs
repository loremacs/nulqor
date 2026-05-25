//! Hello World panel — minimal window content demo.

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{EventPattern, ExtensionManifest};

pub struct HelloWorldExtension {
    manifest: ExtensionManifest,
}

impl HelloWorldExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for HelloWorldExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        ctx.bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(|_ev| {
                eprintln!("[hello-world] canvas ready — Hello World UI active");
            }),
        );
        eprintln!("[hello-world] activated");
        Ok(())
    }
}
