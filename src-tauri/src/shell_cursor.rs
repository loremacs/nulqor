//! Client cursor position for click-through hitbox polling when the window ignores cursor events.

use tauri::Window;

/// Cursor position in window **client** coordinates (physical pixels, top-left of webview).
#[tauri::command]
pub fn shell_cursor_client(window: Window) -> Result<(f64, f64), String> {
    let cursor = window.cursor_position().map_err(|e| e.to_string())?;
    let inner = window.inner_position().map_err(|e| e.to_string())?;
    Ok((
        cursor.x as f64 - inner.x as f64,
        cursor.y as f64 - inner.y as f64,
    ))
}
