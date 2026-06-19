//! Global Workbench enable/disable preferences (`.nulqor/workbench-prefs.json`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

pub const PROTECTED_EXTENSIONS: &[&str] = &["host", "registry"];

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkbenchPrefs {
    #[serde(default)]
    pub disabled_extensions: Vec<String>,
    #[serde(default)]
    pub disabled_skills: Vec<String>,
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    #[serde(default)]
    pub disabled_agents: Vec<String>,
}

impl WorkbenchPrefs {
    pub fn extension_enabled(&self, id: &str) -> bool {
        !self.disabled_extensions.iter().any(|x| x == id)
    }

    pub fn skill_enabled(&self, name: &str) -> bool {
        !self.disabled_skills.iter().any(|x| x == name)
    }

    pub fn rule_enabled(&self, filename: &str) -> bool {
        !self.disabled_rules.iter().any(|x| x == filename)
    }

    pub fn agent_enabled(&self, name: &str) -> bool {
        !self.disabled_agents.iter().any(|x| x == name)
    }

    pub fn set_extension_enabled(&mut self, id: &str, enabled: bool) {
        if is_protected_extension(id) {
            return;
        }
        self.disabled_extensions.retain(|x| x != id);
        if !enabled {
            self.disabled_extensions.push(id.to_owned());
        }
    }

    pub fn set_skill_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_skills.retain(|x| x != name);
        if !enabled {
            self.disabled_skills.push(name.to_owned());
        }
    }

    pub fn set_rule_enabled(&mut self, filename: &str, enabled: bool) {
        self.disabled_rules.retain(|x| x != filename);
        if !enabled {
            self.disabled_rules.push(filename.to_owned());
        }
    }

    pub fn set_agent_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_agents.retain(|x| x != name);
        if !enabled {
            self.disabled_agents.push(name.to_owned());
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

pub fn is_protected_extension(id: &str) -> bool {
    PROTECTED_EXTENSIONS.contains(&id)
}

pub fn prefs_path(root: &Path) -> PathBuf {
    root.join(".nulqor").join("workbench-prefs.json")
}

pub fn resolve_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|e| {
        eprintln!("[nulqor] current_dir() failed ({:?}), using manifest dir as workspace root", e);
        PathBuf::from(".")
    });
    if let Some(parent) = cwd.parent() {
        if parent.join("extensions").exists() || parent.join("AGENTS.md").exists() {
            return parent.to_path_buf();
        }
    }
    cwd
}

pub fn load(root: &Path) -> WorkbenchPrefs {
    let path = prefs_path(root);
    if !path.exists() {
        return WorkbenchPrefs::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save(root: &Path, prefs: &WorkbenchPrefs) -> Result<(), CoreError> {
    let path = prefs_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CoreError::Io(format!("create {}: {e}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(prefs)
        .map_err(|e| CoreError::Io(format!("serialize workbench prefs: {e}")))?;
    std::fs::write(&path, json)
        .map_err(|e| CoreError::Io(format!("write {}: {e}", path.display())))
}

pub fn is_rules_index_file(filename: &str) -> bool {
    Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| stem.eq_ignore_ascii_case("index"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rules_index_case_insensitive() {
        assert!(is_rules_index_file("index.md"));
        assert!(is_rules_index_file("INDEX.md"));
        assert!(!is_rules_index_file("current-date.md"));
    }

    #[test]
    fn protected_extensions_cannot_be_disabled() {
        let mut prefs = WorkbenchPrefs::default();
        prefs.set_extension_enabled("host", false);
        assert!(prefs.extension_enabled("host"));
    }
}
