//! Workbench panel — inspect extensions, commands, skills, rules, and agents.

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{EventPattern, ExtensionManifest};

pub struct WorkbenchExtension {
    manifest: ExtensionManifest,
}

impl WorkbenchExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for WorkbenchExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        ctx.bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(|_ev| {
                eprintln!("[workbench] canvas ready — workbench UI available from Apps menu");
            }),
        );
        eprintln!("[workbench] activated");
        Ok(())
    }
}
