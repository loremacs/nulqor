//! macOS native menu bar.
//!
//! Mirrors the webview Settings / Layout / Apps dropdowns in the system menu bar.
//! Menu activations are forwarded to the frontend as "nulqor:menu-action" events.
//! The frontend calls `update_menu_check` / `update_window_mode_label` to keep
//! check items in sync with JS state.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Wry};

use crate::startup_config::ShellConfig;

pub struct NativeMenuHandles {
    click_through: CheckMenuItem<Wry>,
    always_on_top: CheckMenuItem<Wry>,
    grid_mode: CheckMenuItem<Wry>,
    split_mode: CheckMenuItem<Wry>,
    snap_enabled: CheckMenuItem<Wry>,
    show_grid: CheckMenuItem<Wry>,
    window_toggle: MenuItem<Wry>,
    panel_checks: Vec<(String, CheckMenuItem<Wry>)>,
}

pub struct NativeMenuState(pub Mutex<Option<NativeMenuHandles>>);

pub fn build_and_install(
    app: &AppHandle,
    open_panels: &[String],
    all_panels: &[String],
    shell: &ShellConfig,
) -> tauri::Result<()> {
    // Settings
    let click_through = CheckMenuItem::with_id(
        app, "settings:click_through", "Click Through Desktop",
        true, shell.click_through, None::<&str>,
    )?;
    let always_on_top = CheckMenuItem::with_id(
        app, "settings:always_on_top", "Always on Top",
        true, shell.always_on_top, None::<&str>,
    )?;
    let workbench_reset = MenuItem::with_id(
        app, "settings:workbench_reset", "Reset Workbench Toggles",
        true, None::<&str>,
    )?;
    let settings_sub = Submenu::with_items(app, "Settings", true, &[
        &click_through,
        &always_on_top,
        &PredefinedMenuItem::separator(app)?,
        &workbench_reset,
    ])?;

    // Layout
    let grid_mode = CheckMenuItem::with_id(
        app, "layout:grid", "Grid Mode", true, true, None::<&str>,
    )?;
    let split_mode = CheckMenuItem::with_id(
        app, "layout:split", "Split Mode", true, false, None::<&str>,
    )?;
    let snap_enabled = CheckMenuItem::with_id(
        app, "layout:snap", "Snap to Grid", true, shell.snap_enabled, None::<&str>,
    )?;
    let show_grid = CheckMenuItem::with_id(
        app, "layout:show_grid", "Show Grid", true, shell.show_grid, None::<&str>,
    )?;
    let layout_sub = Submenu::with_items(app, "Layout", true, &[
        &grid_mode,
        &split_mode,
        &PredefinedMenuItem::separator(app)?,
        &snap_enabled,
        &show_grid,
    ])?;

    // Apps (one checkable item per panel extension)
    let panel_check_items: Vec<CheckMenuItem<Wry>> = all_panels
        .iter()
        .map(|id| CheckMenuItem::with_id(
            app,
            format!("apps:{id}"),
            id.as_str(),
            true,
            open_panels.contains(id),
            None::<&str>,
        ))
        .collect::<Result<Vec<_>, _>>()?;

    let apps_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> =
        panel_check_items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<Wry>).collect();

    let apps_sub = if apps_refs.is_empty() {
        Submenu::new(app, "Apps", true)?
    } else {
        Submenu::with_items(app, "Apps", true, apps_refs.as_slice())?
    };

    // Window
    let window_toggle = MenuItem::with_id(
        app, "window:toggle_mode", "Enter Fullscreen", true, None::<&str>,
    )?;
    let window_sub = Submenu::with_items(app, "Window", true, &[
        &PredefinedMenuItem::minimize(app, None)?,
        &PredefinedMenuItem::separator(app)?,
        &window_toggle,
    ])?;

    // App menu (macOS: first entry uses the app name)
    let app_sub = Submenu::with_items(app, "Nulqor", true, &[
        &PredefinedMenuItem::about(app, None, None)?,
        &PredefinedMenuItem::separator(app)?,
        &PredefinedMenuItem::quit(app, None)?,
    ])?;

    let menu = Menu::with_items(app, &[
        &app_sub,
        &settings_sub,
        &layout_sub,
        &apps_sub,
        &window_sub,
    ])?;
    app.set_menu(menu)?;

    let panel_checks = all_panels.iter().cloned().zip(panel_check_items).collect();
    app.manage(NativeMenuState(Mutex::new(Some(NativeMenuHandles {
        click_through,
        always_on_top,
        grid_mode,
        split_mode,
        snap_enabled,
        show_grid,
        window_toggle,
        panel_checks,
    }))));

    Ok(())
}

/// Keep a check item in sync with the frontend's current state.
#[tauri::command]
pub fn update_menu_check(app: AppHandle, id: String, checked: bool) -> Result<(), String> {
    let Some(state) = app.try_state::<NativeMenuState>() else { return Ok(()); };
    let Ok(guard) = state.0.lock() else { return Ok(()); };
    let Some(handles) = guard.as_ref() else { return Ok(()); };

    match id.as_str() {
        "settings:click_through" => {
            if let Err(e) = handles.click_through.set_checked(checked) {
                eprintln!("[nulqor] native menu update failed: {:?}", e);
            }
        }
        "settings:always_on_top" => {
            if let Err(e) = handles.always_on_top.set_checked(checked) {
                eprintln!("[nulqor] native menu update failed: {:?}", e);
            }
        }
        "layout:grid" => {
            if let Err(e) = handles.grid_mode.set_checked(checked) {
                eprintln!("[nulqor] native menu update failed: {:?}", e);
            }
        }
        "layout:split" => {
            if let Err(e) = handles.split_mode.set_checked(checked) {
                eprintln!("[nulqor] native menu update failed: {:?}", e);
            }
        }
        "layout:snap" => {
            if let Err(e) = handles.snap_enabled.set_checked(checked) {
                eprintln!("[nulqor] native menu update failed: {:?}", e);
            }
        }
        "layout:show_grid" => {
            if let Err(e) = handles.show_grid.set_checked(checked) {
                eprintln!("[nulqor] native menu update failed: {:?}", e);
            }
        }
        other if other.starts_with("apps:") => {
            let panel_id = &other["apps:".len()..];
            if let Some((_, item)) = handles.panel_checks.iter().find(|(pid, _)| pid == panel_id) {
                if let Err(e) = item.set_checked(checked) {
                    eprintln!("[nulqor] native menu update failed: {:?}", e);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Updates the Window › toggle item label to reflect the current window mode.
#[tauri::command]
pub fn update_window_mode_label(app: AppHandle, is_fullscreen: bool) -> Result<(), String> {
    let Some(state) = app.try_state::<NativeMenuState>() else { return Ok(()); };
    let Ok(guard) = state.0.lock() else { return Ok(()); };
    let Some(handles) = guard.as_ref() else { return Ok(()); };
    let label = if is_fullscreen { "Enter Windowed Mode" } else { "Enter Fullscreen" };
    if let Err(e) = handles.window_toggle.set_text(label) {
        eprintln!("[nulqor] native menu update failed: {:?}", e);
    }
    Ok(())
}
