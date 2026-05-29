//! IPC bridge — Tauri invoke routing to core commands and event forwarding to the frontend.
//! (DESIGN.md §2.8, BUILD_PLAN §1.8)
//!
//! The frontend MUST go through this bridge; it may not call extension internals directly.
//!
//! Commands exposed to TypeScript:
//! - `core_invoke`       — invoke a versioned command by id (runs on blocking pool — never freezes UI)
//! - `core_list_commands` — introspect available commands (debugging / dev tools)

use tauri::State;

use crate::context::AppState;
use crate::types::CommandId;

/// Invoke a core command by its versioned id.
///
/// `id`    — command id string, e.g. `"hello:ping@1"`
/// `input` — JSON input passed to the command handler
///
/// Returns the JSON output of the command, or an error string.
///
/// Long-running handlers (e.g. LM Studio model load) run on the async runtime's
/// blocking thread pool so the WebView / window chrome stay responsive.
#[tauri::command]
pub async fn core_invoke(
    id: String,
    input: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let cmd_id =
        CommandId::parse(&id).map_err(|e| format!("invalid command id '{id}': {e}"))?;
    let commands = state.core.commands.clone();
    tauri::async_runtime::spawn_blocking(move || commands.invoke("frontend", &cmd_id, input))
        .await
        .map_err(|e| format!("command task failed: {e}"))?
        .map_err(|e| e.to_string())
}

/// List all registered command ids. Useful for dev tooling.
#[tauri::command]
pub fn core_list_commands(state: State<'_, AppState>) -> Vec<String> {
    let mut cmds = state.core.commands.list_commands();
    cmds.sort();
    cmds
}
