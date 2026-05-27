//! In-repo extension implementations (statically compiled, Phase 1â€“3).
//! Source lives under `extensions/<id>/src/lib.rs`; manifests in `extension.toml`.

#[path = "../../../extensions/host/src/lib.rs"]
pub mod host;
#[path = "../../../extensions/hello-panel/src/lib.rs"]
pub mod hello_panel;
#[path = "../../../extensions/hello-world/src/lib.rs"]
pub mod hello_world;
#[path = "../../../extensions/provider-lmstudio/src/lib.rs"]
pub mod provider_lmstudio;
#[path = "../../../extensions/transcript/src/lib.rs"]
pub mod transcript;
#[path = "../../../extensions/http-api/src/lib.rs"]
pub mod http_api;
#[path = "../../../extensions/chat-panel/src/lib.rs"]
pub mod chat_panel;
#[path = "../../../extensions/context-editor/src/lib.rs"]
pub mod context_editor;
#[path = "../../../extensions/mcp-bridge/src/lib.rs"]
pub mod mcp_bridge;
#[path = "../../../extensions/skill-runner/src/lib.rs"]
pub mod skill_runner;
#[path = "../../../extensions/validation/src/lib.rs"]
pub mod validation;
#[path = "../../../extensions/session-store/src/lib.rs"]
pub mod session_store;
#[path = "../../../extensions/run-logger/src/lib.rs"]
pub mod run_logger;

#[path = "../../../extensions/clock-panel/src/lib.rs"]
pub mod clock_panel;

