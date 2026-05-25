//! Chat panel extension — Phase 2.4 (BUILD_PLAN §2.4, decisions/006 §4, §6).
//!
//! The Rust side: registers the panel and subscribes to transcript / provider events,
//! forwarding them to the TypeScript frontend via Tauri's event bus.
//!
//! The TypeScript UI lives in `src/panels/chat.ts` and is the primary
//! conversational interface: connection bar, model/agent selectors, transcript
//! view with per-turn streaming, participant labels, collapsible reasoning blocks,
//! and harness token cost display.

use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{EventPattern, ExtensionManifest};

pub struct ChatPanelExtension {
    manifest: ExtensionManifest,
}

impl ChatPanelExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for ChatPanelExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        // Subscribe to canvas:ready@1 so the panel mounts only after the canvas is up.
        ctx.bus.subscribe(
            EventPattern::exact("canvas", "ready", 1),
            Arc::new(|_ev| {
                eprintln!("[chat-panel] canvas ready — panel would mount TypeScript UI here");
            }),
        );

        // The panel listens for transcript events to drive live updates.
        // In the Tauri WebView these are forwarded by the IPC bridge via Tauri's emit().
        // Phase 2 wires this through the existing event bus; full Tauri::emit() integration
        // is added when the TypeScript UI consumes it.
        ctx.bus.subscribe(
            EventPattern::namespace("transcript"),
            Arc::new(|ev| {
                eprintln!("[chat-panel] transcript event: {}", ev.id.name);
            }),
        );
        ctx.bus.subscribe(
            EventPattern::namespace("provider"),
            Arc::new(|ev| {
                eprintln!("[chat-panel] provider event: {}", ev.id.name);
            }),
        );

        eprintln!("[chat-panel] activated — TypeScript UI loads from src/panels/chat.ts");
        Ok(())
    }
}
