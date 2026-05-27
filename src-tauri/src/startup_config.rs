//! Reads `nulqor.toml` at the workspace root for startup extensions, shell, and open panels.

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct StartupConfig {
    pub open_panels: Vec<String>,
    pub shell: ShellConfig,
    pub enabled_extensions: Option<HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    #[serde(default = "default_cell_pixels")]
    pub cell_pixels: u32,
    #[serde(default = "default_true")]
    pub snap_enabled: bool,
    #[serde(default = "default_true")]
    pub show_grid: bool,
    #[serde(default = "default_true")]
    pub sash_snap_enabled: bool,
    #[serde(default = "default_true")]
    pub click_through: bool,
    #[serde(default = "default_false")]
    pub always_on_top: bool,
}

fn default_false() -> bool {
    false
}

#[derive(Debug, Clone, Serialize)]
pub struct PanelMeta {
    pub id: String,
    pub kind: String,
}

#[derive(Debug, Deserialize)]
struct StartupToml {
    open_panels: Option<Vec<String>>,
    entry_panel: Option<String>,
    enabled_extensions: Option<Vec<String>>,
    shell: Option<ShellToml>,
}

#[derive(Debug, Deserialize)]
struct ShellToml {
    #[serde(alias = "grid_size", alias = "grid_cols", alias = "grid_rows")]
    _legacy_grid: Option<u32>,
    cell_pixels: Option<u32>,
    snap_enabled: Option<bool>,
    show_grid: Option<bool>,
    sash_snap_enabled: Option<bool>,
    click_through: Option<bool>,
    always_on_top: Option<bool>,
}

fn default_cell_pixels() -> u32 {
    64
}

fn default_true() -> bool {
    true
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            cell_pixels: default_cell_pixels(),
            snap_enabled: true,
            show_grid: true,
            sash_snap_enabled: true,
            click_through: true,
            always_on_top: false,
        }
    }
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            open_panels: vec!["hello-world".into()],
            shell: ShellConfig::default(),
            enabled_extensions: None,
        }
    }
}

fn parse_shell(raw: Option<ShellToml>) -> ShellConfig {
    let Some(raw) = raw else {
        return ShellConfig::default();
    };
    ShellConfig {
        cell_pixels: raw.cell_pixels.unwrap_or_else(default_cell_pixels),
        snap_enabled: raw.snap_enabled.unwrap_or(true),
        show_grid: raw.show_grid.unwrap_or(true),
        sash_snap_enabled: raw.sash_snap_enabled.unwrap_or(true),
        click_through: raw.click_through.unwrap_or(true),
        always_on_top: raw.always_on_top.unwrap_or(false),
    }
}

pub fn load_startup_config(root: &Path) -> StartupConfig {
    let path = root.join("nulqor.toml");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        eprintln!("[CONFIG] nulqor.toml not found — using defaults");
        return StartupConfig::default();
    };

    match toml::from_str::<StartupToml>(&raw) {
        Ok(parsed) => {
            let enabled: Option<HashSet<String>> =
                parsed.enabled_extensions.map(|ids| ids.into_iter().collect());
            let open_panels = parsed
                .open_panels
                .or_else(|| parsed.entry_panel.map(|id| vec![id]))
                .unwrap_or_else(|| vec!["hello-world".into()]);
            let shell = parse_shell(parsed.shell);
            if let Some(ref set) = enabled {
                eprintln!(
                    "[CONFIG] nulqor.toml: open_panels={:?}, cell_pixels={}, enabled={} extensions",
                    open_panels,
                    shell.cell_pixels,
                    set.len()
                );
            } else {
                eprintln!(
                    "[CONFIG] nulqor.toml: open_panels={open_panels:?}, cell_pixels={}",
                    shell.cell_pixels
                );
            }
            StartupConfig {
                open_panels,
                shell,
                enabled_extensions: enabled,
            }
        }
        Err(e) => {
            eprintln!("[CONFIG] nulqor.toml parse error: {e} — using defaults");
            StartupConfig::default()
        }
    }
}

/// Discover Panel-kind extensions on disk, optionally filtered by the enabled set.
pub fn discover_panels(
    extensions_dir: &Path,
    enabled: Option<&HashSet<String>>,
) -> Vec<PanelMeta> {
    let mut panels = Vec::new();
    let Ok(entries) = std::fs::read_dir(extensions_dir) else {
        return panels;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let toml_path = path.join("extension.toml");
        if !toml_path.exists() {
            continue;
        }
        let Ok(raw) = nulqor_lint::parse_manifest(&toml_path) else {
            continue;
        };
        if raw.extension.kind != "Panel" {
            continue;
        }
        let id = raw.extension.id;
        if let Some(enabled) = enabled {
            if !enabled.contains(&id) {
                continue;
            }
        }
        panels.push(PanelMeta {
            id: id.clone(),
            kind: "Panel".into(),
        });
    }

    panels.sort_by(|a, b| a.id.cmp(&b.id));
    panels
}

pub fn canvas_config_json(
    startup: &StartupConfig,
    extensions_dir: &Path,
) -> serde_json::Value {
    let panels = discover_panels(extensions_dir, startup.enabled_extensions.as_ref());
    serde_json::json!({
        "open_panels": startup.open_panels,
        "shell": startup.shell,
        "panels": panels,
        "enabled_extensions": startup
            .enabled_extensions
            .as_ref()
            .map(|set| set.iter().cloned().collect::<Vec<_>>()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "nulqor-startup-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ))
    }

    #[test]
    fn discover_panels_filters_enabled() {
        let root = tmp_dir();
        fs::create_dir_all(root.join("extensions/a/src")).unwrap();
        fs::create_dir_all(root.join("extensions/b/src")).unwrap();
        fs::write(
            root.join("extensions/a/extension.toml"),
            r#"[extension]
id = "a"
version = "0.1.0"
kind = "Panel"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
"#,
        )
        .unwrap();
        fs::write(
            root.join("extensions/b/extension.toml"),
            r#"[extension]
id = "b"
version = "0.1.0"
kind = "Service"
api-version = "v1"
schema-version = "1.0.0"
min-core = "0.1.0"
"#,
        )
        .unwrap();

        let enabled: HashSet<String> = ["a".into()].into_iter().collect();
        let panels = discover_panels(&root.join("extensions"), Some(&enabled));
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].id, "a");
    }

    #[test]
    fn parse_shell_reads_cell_pixels() {
        let shell = parse_shell(Some(ShellToml {
            _legacy_grid: None,
            cell_pixels: Some(48),
            snap_enabled: None,
            show_grid: None,
            sash_snap_enabled: None,
            click_through: None,
            always_on_top: None,
        }));
        assert_eq!(shell.cell_pixels, 48);
    }
}
