//! Clock panel — live time tile for multi-panel canvas testing.

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{EventPattern, ExtensionManifest};

pub struct ClockPanelExtension {
    manifest: ExtensionManifest,
}

impl ClockPanelExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for ClockPanelExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        ctx.bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(|_ev| {
                eprintln!("[clock-panel] canvas ready — clock UI active");
            }),
        );
        eprintln!("[clock-panel] activated");
        Ok(())
    }
}
