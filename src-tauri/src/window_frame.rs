//! Persisted window frame for cold-start geometry (read in setup before show).

use serde::{Deserialize, Serialize};
use tauri::window::Color;
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

const SNAP_ABS_TOLERANCE_PX: i32 = 28;
const WINDOWED_BG: Color = Color(18, 18, 22, 255);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowFramePersist {
    pub mode: String,
    pub width: f64,
    pub height: f64,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub anchor: Option<String>,
    #[serde(default)]
    pub monitor_name: Option<String>,
}

struct WorkArea {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    half_width: i32,
    half_height: i32,
}

struct SnapGeometry {
    x: i32,
    y: i32,
}

pub fn path_for(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| e.to_string())
        .map(|p| p.join("window-frame.json"))
}

pub fn load(app: &AppHandle) -> Option<WindowFramePersist> {
    let path = path_for(app).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save(app: &AppHandle, frame: &WindowFramePersist) -> Result<(), String> {
    let path = path_for(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string(frame).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

fn work_area(monitor: &tauri::Monitor) -> WorkArea {
    let work = monitor.work_area();
    let width = work.size.width;
    let height = work.size.height;
    WorkArea {
        x: work.position.x,
        y: work.position.y,
        width,
        height,
        half_width: (width as i32 + 1) / 2,
        half_height: (height as i32 + 1) / 2,
    }
}

fn near_px(a: i32, b: i32) -> bool {
    (a - b).abs() <= SNAP_ABS_TOLERANCE_PX
}

fn snap_position(anchor: &str, wa: &WorkArea) -> Option<SnapGeometry> {
    match anchor {
        "maximize" => Some(SnapGeometry {
            x: wa.x,
            y: wa.y,
        }),
        "left" => Some(SnapGeometry {
            x: wa.x,
            y: wa.y,
        }),
        "right" => Some(SnapGeometry {
            x: wa.x + wa.half_width,
            y: wa.y,
        }),
        "top" => Some(SnapGeometry {
            x: wa.x,
            y: wa.y,
        }),
        "bottom" => Some(SnapGeometry {
            x: wa.x,
            y: wa.y + wa.half_height,
        }),
        "top-left" => Some(SnapGeometry {
            x: wa.x,
            y: wa.y,
        }),
        "top-right" => Some(SnapGeometry {
            x: wa.x + wa.half_width,
            y: wa.y,
        }),
        "bottom-left" => Some(SnapGeometry {
            x: wa.x,
            y: wa.y + wa.half_height,
        }),
        "bottom-right" => Some(SnapGeometry {
            x: wa.x + wa.half_width,
            y: wa.y + wa.half_height,
        }),
        _ => None,
    }
}

fn resolve_monitor(
    window: &WebviewWindow,
    preferred_name: Option<&str>,
) -> Result<Option<tauri::Monitor>, String> {
    if let Some(name) = preferred_name {
        if let Ok(monitors) = window.available_monitors() {
            if let Some(found) = monitors
                .into_iter()
                .find(|m| m.name().map(|n| n.as_str()) == Some(name))
            {
                return Ok(Some(found));
            }
        }
    }

    if let Ok(Some(current)) = window.current_monitor() {
        return Ok(Some(current));
    }

    if let Ok(Some(primary)) = window.primary_monitor() {
        return Ok(Some(primary));
    }

    Ok(None)
}

fn resolve_geometry(
    window: &WebviewWindow,
    frame: &WindowFramePersist,
) -> Result<(Option<(i32, i32)>, f64, f64), String> {
    let anchor = frame.anchor.as_deref().unwrap_or("free");

    if anchor != "free" {
        if let Some(monitor) = resolve_monitor(window, frame.monitor_name.as_deref())? {
            let wa = work_area(&monitor);
            if let Some(snap) = snap_position(anchor, &wa) {
                return Ok((Some((snap.x, snap.y)), frame.width, frame.height));
            }
        }
    }

    let pos = if frame.x >= 0 && frame.y >= 0 {
        Some((frame.x, frame.y))
    } else {
        None
    };
    Ok((pos, frame.width, frame.height))
}

pub fn apply(window: &WebviewWindow, frame: &WindowFramePersist) -> Result<(), String> {
    let is_fullscreen = window.is_fullscreen().map_err(|e| e.to_string())?;

    if frame.mode == "fullscreen" {
        if !is_fullscreen {
            window.set_resizable(false).map_err(|e| e.to_string())?;
            window.set_fullscreen(true).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    if is_fullscreen {
        window.set_fullscreen(false).map_err(|e| e.to_string())?;
    }
    window.set_resizable(true).map_err(|e| e.to_string())?;

    let (position, width, height) = resolve_geometry(window, frame)?;
    window
        .set_size(LogicalSize::new(width, height))
        .map_err(|e| e.to_string())?;

    if let Some((x, y)) = position {
        window
            .set_position(PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    } else {
        window.center().map_err(|e| e.to_string())?;
    }

    let _ = window.set_background_color(Some(WINDOWED_BG));

    Ok(())
}

#[tauri::command]
pub fn sync_window_frame(app: AppHandle, frame: WindowFramePersist) -> Result<(), String> {
    save(&app, &frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_left_uses_work_area_origin() {
        let wa = WorkArea {
            x: 100,
            y: 50,
            width: 1920,
            height: 1000,
            half_width: 960,
            half_height: 500,
        };
        let snap = snap_position("left", &wa).unwrap();
        assert!(near_px(snap.x, 100));
        assert!(near_px(snap.y, 50));
    }
}
