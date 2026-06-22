//! Nulqor core entry point.
//!
//! Responsibilities: build the eight core objects, create the loader,
//! register in-repo extensions, activate everything, wire Tauri IPC.
//!
//! The eight frozen responsibilities (DESIGN.md §2):
//!   1. loader (loader.rs)        5. capability layer (capability.rs)
//!   2. event bus (events.rs)     6. async runtime owner (runtime.rs)
//!   3. command registry          7. IPC bridge (ipc.rs)
//!      (commands.rs)             8. version manager (version.rs)
//!   4. permission gate
//!      (permission.rs)
//!
//! NOTHING outside this list belongs in the core.

// Scaffold-phase extensions and core APIs are partially wired; silence until Phase 2+.
#![allow(dead_code)]

use std::sync::Arc;

use tauri::{Emitter, Manager};

mod capability;
mod commands;
mod context;
mod error;
mod events;
mod extensions;
mod ipc;
mod loader;
mod native_menu;
mod permission;
mod runtime;
mod shell_cursor;
mod startup_config;
mod types;
mod version;
mod window_frame;
mod workbench_prefs;

use capability::{CapabilityRegistry, Capabilities};
use commands::CommandRegistry;
use context::{AppState, CoreContext, InMemoryConfigStore};
use events::EventBus;
use extensions::{
    chat_panel, clock_panel, context_editor, hello_panel, hello_world, host,
    http_api, mcp_bridge, provider_llamacpp, provider_lmstudio, provider_ollama, provider_router,
    agent_loop, context_manager, decision_records, persistence, registry, run_logger, session_store,
    skill_runner, transcript, validation, workbench,
};
use permission::PermissionGate;
use runtime::Runtime;
use version::VersionManager;

// ---------------------------------------------------------------------------
// Core bootstrap
// ---------------------------------------------------------------------------

fn build_core(_extensions_dir: &std::path::Path) -> CoreContext {
    let permissions = Arc::new(PermissionGate::new());
    CoreContext {
        bus: Arc::new(EventBus::new()),
        commands: Arc::new(CommandRegistry::new(permissions.clone())),
        versions: Arc::new(VersionManager::new()),
        permissions,
        caps: Arc::new(Capabilities::new()),
        capability_registry: Arc::new(CapabilityRegistry::new()),
        runtime: Arc::new(Runtime::new()),
        config: Arc::new(InMemoryConfigStore::new()),
    }
}

fn load_extensions(
    extensions_dir: &std::path::Path,
    ctx: &CoreContext,
    startup: &startup_config::StartupConfig,
) {
    // Repo root is the parent of extensions/
    let root = extensions_dir.parent().unwrap_or(extensions_dir);

    let canvas_config =
        startup_config::canvas_config_json(&startup, extensions_dir);
    if let Err(e) = ctx.config.set("host", "canvas", canvas_config) {
        eprintln!("[CONFIG] failed to store canvas config: {e}");
    }
    if let Err(e) = ctx.config.set(
        "provider-router",
        "active",
        serde_json::json!({ "instance": startup.active_provider }),
    ) {
        eprintln!("[CONFIG] failed to store active provider: {e}");
    }

    if let Some(ref enabled) = startup.enabled_extensions {
        for panel_id in &startup.open_panels {
            if !enabled.contains(panel_id) {
                eprintln!(
                    "[CONFIG] warning: open_panels '{panel_id}' is not in enabled_extensions",
                );
            }
        }
    }

    let mut loader = loader::Loader::new();
    loader.register("host", |m| Arc::new(host::HostExtension::new(m)));
    loader.register("hello-panel", |m| Arc::new(hello_panel::HelloPanelExtension::new(m)));
    loader.register("hello-world", |m| Arc::new(hello_world::HelloWorldExtension::new(m)));
    loader.register("clock-panel", |m| Arc::new(clock_panel::ClockPanelExtension::new(m)));
    loader.register("provider-lmstudio", |m| {
        Arc::new(provider_lmstudio::LmStudioProvider::new(m))
    });
    loader.register("provider-ollama", |m| {
        Arc::new(provider_ollama::OllamaProvider::new(m))
    });
    loader.register("provider-llamacpp", |m| {
        Arc::new(provider_llamacpp::LlamaCppProvider::new(m))
    });
    let active_provider = startup.active_provider.clone();
    loader.register("provider-router", move |m| {
        Arc::new(provider_router::ProviderRouter::new(m, active_provider.clone()))
    });
    loader.register("transcript", |m| Arc::new(transcript::TranscriptExtension::new(m)));
    loader.register("session-store", |m| Arc::new(session_store::SessionStoreExtension::new(m)));
    loader.register("http-api", |m| Arc::new(http_api::HttpApiExtension::new(m)));
    loader.register("chat-panel", |m| Arc::new(chat_panel::ChatPanelExtension::new(m)));
    loader.register("context-editor", |m| {
        Arc::new(context_editor::ContextEditorExtension::new(m))
    });
    loader.register("mcp-bridge", |m| Arc::new(mcp_bridge::McpBridgeExtension::new(m)));
    loader.register("skill-runner", |m| {
        Arc::new(skill_runner::SkillRunnerExtension::new(m))
    });
    loader.register("validation", |m| Arc::new(validation::ValidationExtension::new(m)));
    loader.register("run-logger", |m| Arc::new(run_logger::RunLoggerExtension::new(m)));
    loader.register("registry", |m| Arc::new(registry::RegistryExtension::new(m)));
    loader.register("workbench", |m| Arc::new(workbench::WorkbenchExtension::new(m)));
    loader.register("decision-records", |m| {
        Arc::new(decision_records::DecisionRecordsExtension::new(m))
    });
    loader.register("agent-loop", |m| {
        Arc::new(agent_loop::AgentLoopExtension::new(m))
    });
    loader.register("context-manager", |m| {
        Arc::new(context_manager::ContextManagerExtension::new(m))
    });
    loader.register("persistence", |m| {
        Arc::new(persistence::PersistenceExtension::new(m))
    });

    match loader.scan_and_load(
        extensions_dir,
        root,
        ctx,
        startup.enabled_extensions.as_ref(),
    ) {
        Ok(_) => eprintln!("[CORE] all extensions loaded successfully"),
        Err(e) => eprintln!("[CORE] extension load error: {e}"),
    }

    // Emit canvas:ready@1 AFTER all extensions are activated so every subscriber
    // (including hello-panel) has already registered before the event fires.
    ctx.emit("canvas", "ready", 1, serde_json::json!({}));
    eprintln!("[CORE] emitted canvas:ready@1");
}

// ---------------------------------------------------------------------------
// Tauri entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Some(frame) = window_frame::load(app.handle()) {
                    if let Err(e) = window_frame::apply(&window, &frame) {
                        eprintln!("[window-frame] startup apply failed: {e}");
                    } else {
                        eprintln!(
                            "[window-frame] applied {:?} before show ({}x{} @ {}, {})",
                            frame.mode, frame.width, frame.height, frame.x, frame.y
                        );
                    }
                }
            }

            // Resolve extensions directory:
            //   - production: <resource_dir>/extensions/
            //   - development: <cwd>/extensions/  (workspace root)
            let resource_extensions = app
                .path()
                .resource_dir()
                .ok()
                .map(|p| p.join("extensions"))
                .filter(|p| p.exists());

            let extensions_dir = match resource_extensions {
                Some(dir) => dir,
                None => {
                    // cargo sets CWD to the package root (src-tauri/); the extensions/
                    // dir lives one level up at the workspace root.
                    let cwd = std::env::current_dir()
                        .map_err(|e| format!("cannot determine working directory: {e}"))?;
                    if let Some(parent) = cwd.parent() {
                        let parent_ext = parent.join("extensions");
                        if parent_ext.exists() {
                            parent_ext
                        } else {
                            cwd.join("extensions")
                        }
                    } else {
                        cwd.join("extensions")
                    }
                }
            };

            let root = extensions_dir.parent().unwrap_or(&extensions_dir).to_path_buf();
            let startup = startup_config::load_startup_config(&root);
            let ctx = build_core(&extensions_dir);
            load_extensions(&extensions_dir, &ctx, &startup);

            // Forward provider stream events from the internal Rust bus to the
            // Tauri frontend so TypeScript can subscribe with listen().
            {
                use crate::types::{EventPattern, NamespacedEvent};
                let handle = app.handle().clone();
                ctx.bus.subscribe(
                    EventPattern::exact("provider", "stream-delta", 1),
                    Arc::new(move |ev: &NamespacedEvent| {
                        let _ = handle.emit("nulqor:stream-delta", &ev.payload);
                    }),
                );
                let handle = app.handle().clone();
                ctx.bus.subscribe(
                    EventPattern::exact("provider", "stream-done", 1),
                    Arc::new(move |ev: &NamespacedEvent| {
                        let _ = handle.emit("nulqor:stream-done", &ev.payload);
                    }),
                );
            }

            // macOS: build the native menu bar and forward activations to JS.
            #[cfg(target_os = "macos")]
            {
                let panels = startup_config::discover_panels(
                    &extensions_dir,
                    startup.enabled_extensions.as_ref(),
                );
                let all_panel_ids: Vec<String> = panels.iter().map(|p| p.id.clone()).collect();
                if let Err(e) = native_menu::build_and_install(
                    app.handle(),
                    &startup.open_panels,
                    &all_panel_ids,
                    &startup.shell,
                ) {
                    eprintln!("[menu] native menu build failed: {e}");
                }
                app.on_menu_event(|app, event| {
                    let id = event.id().as_ref().to_string();
                    let _ = app.emit("nulqor:menu-action", id);
                });
            }

            app.manage(AppState::new(ctx));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ipc::core_invoke,
            ipc::core_list_commands,
            shell_cursor::shell_cursor_client,
            window_frame::sync_window_frame,
            native_menu::update_menu_check,
            native_menu::update_window_mode_label,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nulqor");
}
