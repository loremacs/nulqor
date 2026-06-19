//! Context editor extension — Phase 2.5 (BUILD_PLAN §2.5, decisions/006 §6, §7, §10).
//!
//! Loads skills, agents, and rules from disk.
//! Assembles the system prompt per decisions/006 §6 order:
//!   1. Agent persona (AGENTS.md or agents/<name>.md)
//!   2. Rules (rules/*.{md,mdc,txt}, alphabetical, INDEX.md skipped)
//!   3. Compact skill index (one line per skill: "- **name**: description")
//!
//! File watcher hot-reloads the skill/agent/rules trees when files change on disk.
//! Skill files use YAML frontmatter (---) as per decisions/006 §11 #10.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::runtime::Runtime;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

// ---------------------------------------------------------------------------
// Skill / Agent / Rule types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct AgentMeta {
    #[allow(dead_code)]
    pub name: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub struct RuleMeta {
    pub filename: String,
    pub excerpt: String,
    pub body: String,
}

// ---------------------------------------------------------------------------
// Context store (hot-reloadable)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ContextStore {
    pub skills: Vec<SkillMeta>,
    pub agents: HashMap<String, AgentMeta>,
    pub rules: Vec<RuleMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextPreferences {
    #[serde(default = "default_active_agent")]
    pub active_agent: String,
    #[serde(default)]
    pub disabled_agents: Vec<String>,
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    #[serde(default)]
    pub disabled_skills: Vec<String>,
}

impl Default for ContextPreferences {
    fn default() -> Self {
        Self {
            active_agent: default_active_agent(),
            disabled_agents: Vec::new(),
            disabled_rules: Vec::new(),
            disabled_skills: Vec::new(),
        }
    }
}

fn default_active_agent() -> String {
    "default".into()
}

impl ContextPreferences {
    fn agent_enabled(&self, name: &str) -> bool {
        !self.disabled_agents.iter().any(|n| n == name)
    }

    fn rule_enabled(&self, filename: &str) -> bool {
        !self.disabled_rules.iter().any(|f| f == filename)
    }

    fn skill_enabled(&self, name: &str) -> bool {
        !self.disabled_skills.iter().any(|n| n == name)
    }

    fn set_agent_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_agents.retain(|n| n != name);
        if enabled {
            self.active_agent = name.to_owned();
        } else if self.active_agent == name {
            self.disabled_agents.push(name.to_owned());
        }
    }

    fn set_rule_enabled(&mut self, filename: &str, enabled: bool) {
        self.disabled_rules.retain(|f| f != filename);
        if !enabled {
            self.disabled_rules.push(filename.to_owned());
        }
    }

    fn set_skill_enabled(&mut self, name: &str, enabled: bool) {
        self.disabled_skills.retain(|n| n != name);
        if !enabled {
            self.disabled_skills.push(name.to_owned());
        }
    }
}

fn load_workbench_prefs(root: &Path) -> crate::workbench_prefs::WorkbenchPrefs {
    crate::workbench_prefs::load(root)
}

fn effective_agent_enabled(
    global: &crate::workbench_prefs::WorkbenchPrefs,
    session: &ContextPreferences,
    name: &str,
) -> bool {
    global.agent_enabled(name) && session.agent_enabled(name)
}

fn effective_rule_enabled(
    global: &crate::workbench_prefs::WorkbenchPrefs,
    session: &ContextPreferences,
    filename: &str,
) -> bool {
    global.rule_enabled(filename) && session.rule_enabled(filename)
}

fn effective_skill_enabled(
    global: &crate::workbench_prefs::WorkbenchPrefs,
    session: &ContextPreferences,
    name: &str,
) -> bool {
    global.skill_enabled(name) && session.skill_enabled(name)
}

fn legacy_preferences_path(root: &Path) -> PathBuf {
    root.join(".nulqor").join("context-preferences.json")
}

fn session_context_path(root: &Path, session_id: &str) -> PathBuf {
    root.join(".nulqor")
        .join("sessions")
        .join(format!("{session_id}.context.json"))
}

fn load_session_preferences(root: &Path, session_id: &str) -> ContextPreferences {
    let path = session_context_path(root, session_id);
    if path.exists() {
        return std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
    }

    let legacy = legacy_preferences_path(root);
    if legacy.exists() {
        let prefs = std::fs::read_to_string(&legacy)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        let _ = save_session_preferences(root, session_id, &prefs);
        return prefs;
    }

    ContextPreferences::default()
}

fn save_session_preferences(
    root: &Path,
    session_id: &str,
    prefs: &ContextPreferences,
) -> Result<(), CoreError> {
    let path = session_context_path(root, session_id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CoreError::Io(format!("create {}: {e}", parent.display())))?;
    }
    let json = serde_json::to_string_pretty(prefs)
        .map_err(|e| CoreError::Io(format!("serialize session context: {e}")))?;
    std::fs::write(&path, json)
        .map_err(|e| CoreError::Io(format!("write {}: {e}", path.display())))
}

fn resolve_session_id(
    cmds: &crate::commands::CommandRegistry,
    input: &serde_json::Value,
) -> Result<String, CoreError> {
    if let Some(id) = input.get("session_id").and_then(|v| v.as_str()) {
        if !id.is_empty() {
            return Ok(id.to_owned());
        }
    }
    let result = cmds.invoke(
        "session-store",
        &CommandId::parse("sessions:active@1")
            .map_err(|e| CoreError::Io(format!("sessions:active parse: {e}")))?,
        serde_json::json!({}),
    )?;
    result
        .get("session_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| CoreError::Io("no active session".into()))
}

fn agent_source_path(name: &str) -> String {
    if name == "default" {
        "AGENTS.md".into()
    } else {
        format!("agents/{name}.md")
    }
}

fn skill_source_path(name: &str) -> String {
    format!("skills/{name}/SKILL.md")
}

fn is_kebab_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

fn validate_rule_filename(filename: &str) -> Result<(), CoreError> {
    if filename.contains('/') || filename.contains('\\') {
        return Err(CoreError::Io("rule filename must not contain path separators".into()));
    }
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    if stem == "INDEX" {
        return Err(CoreError::Io("cannot write rules/INDEX.*".into()));
    }
    let ext = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !matches!(ext, "md" | "mdc" | "txt") {
        return Err(CoreError::Io(
            "rule filename must end with .md, .mdc, or .txt".into(),
        ));
    }
    Ok(())
}

fn write_skill(root: &Path, name: &str, body: &str) -> Result<String, CoreError> {
    if !is_kebab_case(name) {
        return Err(CoreError::Io(format!("invalid skill name '{name}'")));
    }
    let dir = root.join("skills").join(name);
    std::fs::create_dir_all(&dir)
        .map_err(|e| CoreError::Io(format!("create {}: {e}", dir.display())))?;
    let path = dir.join("SKILL.md");
    std::fs::write(&path, body)
        .map_err(|e| CoreError::Io(format!("write {}: {e}", path.display())))?;
    Ok(skill_source_path(name))
}

fn write_rule(root: &Path, filename: &str, body: &str) -> Result<String, CoreError> {
    validate_rule_filename(filename)?;
    let path = root.join("rules").join(filename);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CoreError::Io(format!("create {}: {e}", parent.display())))?;
    }
    std::fs::write(&path, body)
        .map_err(|e| CoreError::Io(format!("write {}: {e}", path.display())))?;
    Ok(format!("rules/{filename}"))
}

fn write_agent(root: &Path, name: &str, body: &str) -> Result<String, CoreError> {
    if name == "default" {
        let path = root.join("AGENTS.md");
        std::fs::write(&path, body)
            .map_err(|e| CoreError::Io(format!("write {}: {e}", path.display())))?;
        return Ok("AGENTS.md".into());
    }
    if !is_kebab_case(name) {
        return Err(CoreError::Io(format!("invalid agent name '{name}'")));
    }
    let agents_dir = root.join("agents");
    std::fs::create_dir_all(&agents_dir)
        .map_err(|e| CoreError::Io(format!("create {}: {e}", agents_dir.display())))?;
    let path = agents_dir.join(format!("{name}.md"));
    std::fs::write(&path, body)
        .map_err(|e| CoreError::Io(format!("write {}: {e}", path.display())))?;
    Ok(agent_source_path(name))
}

fn reload_store(store: &Arc<RwLock<ContextStore>>) {
    let root = resolve_workspace_root();
    let data = ContextStore::load_from(&root);
    let mut guard = store.write().unwrap_or_else(|p| p.into_inner());
    *guard = data;
}

impl ContextStore {
    fn load_from(root: &Path) -> Self {
        let mut store = ContextStore::default();

        // Skills: skills/<name>/SKILL.md (preferred) or skill.md — YAML frontmatter
        let skills_dir = root.join("skills");
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                if !entry.path().is_dir() {
                    continue;
                }
                let skill_md = skill_file_path(&entry.path());
                if let Some(path) = skill_md {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Some(meta) = parse_skill_frontmatter(&content) {
                            store.skills.push(meta);
                        }
                    }
                }
            }
        }
        store.skills.sort_by(|a, b| a.name.cmp(&b.name));

        // Agents: AGENTS.md (default) + agents/<name>.md
        let agents_md = root.join("AGENTS.md");
        if let Ok(body) = std::fs::read_to_string(&agents_md) {
            store
                .agents
                .insert("default".into(), AgentMeta { name: "default".into(), body });
        }
        let agents_dir = root.join("agents");
        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    let name =
                        path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    if let Ok(body) = std::fs::read_to_string(&path) {
                        store.agents.insert(name.clone(), AgentMeta { name, body });
                    }
                }
            }
        }

        // Rules: rules/*.{md,mdc,txt}, alphabetical, skip INDEX.md
        let rules_dir = root.join("rules");
        if let Ok(mut entries) = std::fs::read_dir(&rules_dir).map(|e| {
            e.flatten()
                .filter(|e| {
                    let p = e.path();
                    let filename = p
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    matches!(ext, "md" | "mdc" | "txt")
                        && !crate::workbench_prefs::is_rules_index_file(&filename)
                })
                .collect::<Vec<_>>()
        }) {
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let filename = entry.file_name().to_string_lossy().to_string();
                if let Ok(body) = std::fs::read_to_string(entry.path()) {
                    let excerpt = body.lines().next().unwrap_or("").to_string();
                    store.rules.push(RuleMeta { filename, excerpt, body });
                }
            }
        }

        store
    }

    /// Assemble system prompt per decisions/006 §6, honoring user toggles.
    pub fn assemble_system_prompt(
        &self,
        agent: Option<&str>,
        prefs: &ContextPreferences,
        global: &crate::workbench_prefs::WorkbenchPrefs,
    ) -> String {
        let agent_key = agent.unwrap_or(&prefs.active_agent);
        let mut parts: Vec<String> = Vec::new();

        // 1. Agent persona
        if !agent_key.is_empty()
            && effective_agent_enabled(global, prefs, agent_key)
        {
            if let Some(a) = self.agents.get(agent_key) {
                parts.push(interpolate_date(&a.body));
            }
        }

        // 2. Rules (concatenated in alphabetical order, date placeholders resolved)
        let active_rules: Vec<_> = self
            .rules
            .iter()
            .filter(|r| effective_rule_enabled(global, prefs, &r.filename))
            .collect();
        if !active_rules.is_empty() {
            parts.push(
                active_rules
                    .iter()
                    .map(|r| interpolate_date(&r.body))
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            );
        }

        // 3. Compact skill index
        let active_skills: Vec<_> = self
            .skills
            .iter()
            .filter(|s| effective_skill_enabled(global, prefs, &s.name))
            .collect();
        if !active_skills.is_empty() {
            let index = active_skills
                .iter()
                .map(|s| format!("- **{}**: {}", s.name, s.description))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("## Available Skills\n{index}"));
        }

        parts.join("\n\n---\n\n")
    }

    fn build_context_profile(
        &self,
        session_id: &str,
        prefs: &ContextPreferences,
        global: &crate::workbench_prefs::WorkbenchPrefs,
    ) -> serde_json::Value {
        let prompt = self.assemble_system_prompt(None, prefs, global);
        let agents: Vec<serde_json::Value> = {
            let mut names: Vec<_> = self.agents.keys().cloned().collect();
            names.sort();
            names
                .into_iter()
                .map(|name| {
                    let excerpt = self
                        .agents
                        .get(&name)
                        .map(|a| a.body.lines().next().unwrap_or("").to_string())
                        .unwrap_or_default();
                    let enabled = effective_agent_enabled(global, prefs, &name)
                        && name == prefs.active_agent;
                    serde_json::json!({
                        "name": name,
                        "path": agent_source_path(&name),
                        "excerpt": excerpt,
                        "enabled": enabled,
                    })
                })
                .collect()
        };
        let rules: Vec<serde_json::Value> = self
            .rules
            .iter()
            .map(|r| {
                serde_json::json!({
                    "filename": r.filename,
                    "path": format!("rules/{}", r.filename),
                    "excerpt": r.excerpt,
                    "enabled": effective_rule_enabled(global, prefs, &r.filename),
                })
            })
            .collect();
        let skills: Vec<serde_json::Value> = self
            .skills
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "path": skill_source_path(&s.name),
                    "description": s.description,
                    "enabled": effective_skill_enabled(global, prefs, &s.name),
                })
            })
            .collect();
        serde_json::json!({
            "session_id": session_id,
            "active_agent": prefs.active_agent,
            "token_estimate": (prompt.len() / 4) as u64,
            "agents": agents,
            "rules": rules,
            "skills": skills,
        })
    }
}

/// Replace `{{current_date}}` and `{{current_datetime}}` placeholders with the
/// real UTC date/time at prompt-assembly time. This is the mechanism that makes
/// the temporal-date rule (`rules/current-date.md`) work correctly.
fn interpolate_date(text: &str) -> String {
    let now = chrono::Utc::now();
    text.replace("{{current_date}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{current_datetime}}", &now.format("%Y-%m-%d %H:%M UTC").to_string())
}

fn skill_file_path(skill_dir: &Path) -> Option<PathBuf> {
    let uppercase = skill_dir.join("SKILL.md");
    if uppercase.exists() {
        return Some(uppercase);
    }
    let lowercase = skill_dir.join("skill.md");
    if lowercase.exists() {
        return Some(lowercase);
    }
    None
}

fn parse_skill_frontmatter(content: &str) -> Option<SkillMeta> {
    #[derive(Deserialize)]
    struct Front {
        name: String,
        description: String,
        #[serde(default)]
        triggers: Vec<String>,
    }

    let stripped = content.trim_start_matches("---\n").trim_start_matches("---\r\n");
    let end = stripped.find("\n---").or_else(|| stripped.find("\r\n---"))?;
    let front_yaml = &stripped[..end];
    let rest_start = end + stripped[end..].find('\n')? + 1;
    let body = stripped[rest_start..].trim_start_matches('\n').to_owned();

    let front: Front = match serde_yaml::from_str(front_yaml) {
        Ok(f) => f,
        Err(err) => {
            eprintln!("Failed to parse skill frontmatter: {:?}", err);
            return None;
        }
    };
    Some(SkillMeta {
        name: front.name,
        description: front.description,
        triggers: front.triggers,
        body: format!("---\n{front_yaml}\n---\n{body}"),
    })
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

pub struct ContextEditorExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
    store: Arc<RwLock<ContextStore>>,
}

impl ContextEditorExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self {
            manifest,
            store: Arc::new(RwLock::new(ContextStore::default())),
        }
    }
}

impl Extension for ContextEditorExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let store = self.store.clone();
        let runtime = ctx.runtime.clone();

        // Initial load — determine root from CWD (dev) or parent of src-tauri/ (workspace root)
        let root = resolve_workspace_root();
        {
            let loaded = ContextStore::load_from(&root);
            *store.write().unwrap() = loaded;
        }

        self.register_commands(ctx, root.clone())?;
        self.start_watcher(root, store, runtime);

        eprintln!("[context-editor] activated");
        Ok(())
    }
}

fn resolve_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    // In dev mode cargo sets CWD to src-tauri/; workspace root is one level up
    if let Some(parent) = cwd.parent() {
        if parent.join("extensions").exists() || parent.join("AGENTS.md").exists() {
            return parent.to_path_buf();
        }
    }
    cwd
}

impl ContextEditorExtension {
    fn register_commands(&self, ctx: &CoreContext, root: PathBuf) -> Result<(), CoreError> {
        let store = self.store.clone();
        let cmds = ctx.commands.clone();

        // reload
        {
            let s = store.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "reload".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "skills": "number", "agents": "number", "rules": "number" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |_| {
                    let root = resolve_workspace_root();
                    let loaded = ContextStore::load_from(&root);
                    let (skill_count, agent_count, rule_count) =
                        (loaded.skills.len(), loaded.agents.len(), loaded.rules.len());
                    *s.write().unwrap() = loaded;
                    Ok(serde_json::json!({
                        "skills": skill_count,
                        "agents": agent_count,
                        "rules": rule_count,
                    }))
                }),
            )?;
        }

        // list-skills
        {
            let s = store.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "list-skills".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "skills": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let global = load_workbench_prefs(&root);
                    let skills = s
                        .read()
                        .unwrap()
                        .skills
                        .iter()
                        .map(|sk| {
                            serde_json::json!({
                                "name": sk.name,
                                "description": sk.description,
                                "triggers": sk.triggers,
                                "enabled": global.skill_enabled(&sk.name),
                            })
                        })
                        .collect::<Vec<_>>();
                    Ok(serde_json::json!({ "skills": skills }))
                }),
            )?;
        }

        // list-agents
        {
            let s = store.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "list-agents".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "agents": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let global = load_workbench_prefs(&root);
                    let agents = s
                        .read()
                        .unwrap()
                        .agents
                        .keys()
                        .map(|k| {
                            serde_json::json!({
                                "name": k,
                                "enabled": global.agent_enabled(k),
                            })
                        })
                        .collect::<Vec<_>>();
                    Ok(serde_json::json!({ "agents": agents }))
                }),
            )?;
        }

        // list-rules
        {
            let s = store.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "list-rules".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "rules": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let global = load_workbench_prefs(&root);
                    let rules = s
                        .read()
                        .unwrap()
                        .rules
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "filename": r.filename,
                                "excerpt": r.excerpt,
                                "enabled": global.rule_enabled(&r.filename),
                            })
                        })
                        .collect::<Vec<_>>();
                    Ok(serde_json::json!({ "rules": rules }))
                }),
            )?;
        }

        // load-skill
        {
            let s = store.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "load-skill".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "name": "string" }"#.into(),
                    output_schema: r#"{ "name": "string", "description": "string", "body": "string" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let name = input["name"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("load-skill: 'name' required".into()))?
                        .to_owned();
                    let skill = s
                        .read()
                        .unwrap()
                        .skills
                        .iter()
                        .find(|sk| sk.name == name)
                        .map(|sk| {
                            serde_json::json!({
                                "name": sk.name,
                                "description": sk.description,
                                "body": sk.body,
                            })
                        })
                        .ok_or_else(|| CoreError::Io(format!("skill '{name}' not found")))?;
                    Ok(skill)
                }),
            )?;
        }

        // load-rule
        {
            let s = store.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "load-rule".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "filename": "string" }"#.into(),
                    output_schema: r#"{ "filename": "string", "excerpt": "string", "body": "string" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let filename = input["filename"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("load-rule: 'filename' required".into()))?
                        .to_owned();
                    let rule = s
                        .read()
                        .unwrap()
                        .rules
                        .iter()
                        .find(|r| r.filename == filename)
                        .map(|r| {
                            serde_json::json!({
                                "filename": r.filename,
                                "excerpt": r.excerpt,
                                "body": r.body,
                            })
                        })
                        .ok_or_else(|| CoreError::Io(format!("rule '{filename}' not found")))?;
                    Ok(rule)
                }),
            )?;
        }

        // load-agent
        {
            let s = store.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "load-agent".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "name": "string" }"#.into(),
                    output_schema: r#"{ "name": "string", "path": "string", "body": "string" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let name = input["name"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("load-agent: 'name' required".into()))?
                        .to_owned();
                    let agent = s
                        .read()
                        .unwrap()
                        .agents
                        .get(&name)
                        .map(|a| {
                            serde_json::json!({
                                "name": name,
                                "path": agent_source_path(&name),
                                "body": a.body,
                            })
                        })
                        .ok_or_else(|| CoreError::Io(format!("agent '{name}' not found")))?;
                    Ok(agent)
                }),
            )?;
        }

        // save-skill
        {
            let s = store.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "save-skill".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "name": "string", "body": "string" }"#.into(),
                    output_schema: r#"{ "name": "string", "path": "string" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let name = input["name"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("save-skill: 'name' required".into()))?
                        .to_owned();
                    let body = input["body"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("save-skill: 'body' required".into()))?
                        .to_owned();
                    let path = write_skill(&root, &name, &body)?;
                    reload_store(&s);
                    Ok(serde_json::json!({ "name": name, "path": path }))
                }),
            )?;
        }

        // save-rule
        {
            let s = store.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "save-rule".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "filename": "string", "body": "string" }"#.into(),
                    output_schema: r#"{ "filename": "string", "path": "string" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let filename = input["filename"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("save-rule: 'filename' required".into()))?
                        .to_owned();
                    let body = input["body"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("save-rule: 'body' required".into()))?
                        .to_owned();
                    let path = write_rule(&root, &filename, &body)?;
                    reload_store(&s);
                    Ok(serde_json::json!({ "filename": filename, "path": path }))
                }),
            )?;
        }

        // save-agent
        {
            let s = store.clone();
            let root = root.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "save-agent".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "name": "string", "body": "string" }"#.into(),
                    output_schema: r#"{ "name": "string", "path": "string" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let name = input["name"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("save-agent: 'name' required".into()))?
                        .to_owned();
                    let body = input["body"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("save-agent: 'body' required".into()))?
                        .to_owned();
                    let path = write_agent(&root, &name, &body)?;
                    reload_store(&s);
                    Ok(serde_json::json!({ "name": name, "path": path }))
                }),
            )?;
        }

        // context-profile
        {
            let s = store.clone();
            let root = root.clone();
            let cmds = cmds.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "context-profile".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "session_id": "string?" }"#.into(),
                    output_schema: r#"{ "session_id": "string", "active_agent": "string", "token_estimate": "number", "agents": "array", "rules": "array", "skills": "array" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let session_id = resolve_session_id(&cmds, &input)?;
                    let prefs = load_session_preferences(&root, &session_id);
                    let global = load_workbench_prefs(&root);
                    let store = s.read().unwrap();
                    Ok(store.build_context_profile(&session_id, &prefs, &global))
                }),
            )?;
        }

        // set-context-profile
        {
            let s = store.clone();
            let root = root.clone();
            let cmds = cmds.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "set-context-profile".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "session_id": "string?", "active_agent": "string?", "agent": { "name": "string", "enabled": "boolean" }?, "rule": { "filename": "string", "enabled": "boolean" }?, "skill": { "name": "string", "enabled": "boolean" }? }"#.into(),
                    output_schema: r#"{ "session_id": "string", "active_agent": "string", "token_estimate": "number", "agents": "array", "rules": "array", "skills": "array" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let session_id = resolve_session_id(&cmds, &input)?;
                    let mut prefs = load_session_preferences(&root, &session_id);
                    if let Some(agent) = input.get("active_agent").and_then(|v| v.as_str()) {
                        let store = s.read().unwrap();
                        if store.agents.contains_key(agent) {
                            prefs.active_agent = agent.to_owned();
                            prefs.disabled_agents.retain(|n| n != agent);
                        }
                    }
                    if let Some(agent) = input.get("agent") {
                        if let (Some(name), Some(enabled)) = (
                            agent.get("name").and_then(|v| v.as_str()),
                            agent.get("enabled").and_then(|v| v.as_bool()),
                        ) {
                            let store = s.read().unwrap();
                            if store.agents.contains_key(name) {
                                prefs.set_agent_enabled(name, enabled);
                            }
                        }
                    }
                    if let Some(rule) = input.get("rule") {
                        if let (Some(filename), Some(enabled)) = (
                            rule.get("filename").and_then(|v| v.as_str()),
                            rule.get("enabled").and_then(|v| v.as_bool()),
                        ) {
                            prefs.set_rule_enabled(filename, enabled);
                        }
                    }
                    if let Some(skill) = input.get("skill") {
                        if let (Some(name), Some(enabled)) = (
                            skill.get("name").and_then(|v| v.as_str()),
                            skill.get("enabled").and_then(|v| v.as_bool()),
                        ) {
                            prefs.set_skill_enabled(name, enabled);
                        }
                    }
                    save_session_preferences(&root, &session_id, &prefs)?;
                    let global = load_workbench_prefs(&root);
                    let store = s.read().unwrap();
                    Ok(store.build_context_profile(&session_id, &prefs, &global))
                }),
            )?;
        }

        // system-prompt
        {
            let s = store.clone();
            let root = root.clone();
            let cmds = cmds.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "system-prompt".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "session_id": "string?", "agent": "string?" }"#.into(),
                    output_schema: r#"{ "prompt": "string", "token_estimate": "number" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let session_id = resolve_session_id(&cmds, &input)?;
                    let prefs = load_session_preferences(&root, &session_id);
                    let agent = input["agent"].as_str().map(str::to_owned);
                    let global = load_workbench_prefs(&root);
                    let store = s.read().unwrap();
                    let prompt =
                        store.assemble_system_prompt(agent.as_deref(), &prefs, &global);
                    let token_estimate = (prompt.len() / 4) as u64;
                    Ok(serde_json::json!({ "prompt": prompt, "token_estimate": token_estimate }))
                }),
            )?;
        }

        Ok(())
    }

    fn start_watcher(
        &self,
        root: PathBuf,
        store: Arc<RwLock<ContextStore>>,
        runtime: Arc<Runtime>,
    ) {
        use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

        runtime.spawn_task(Duration::from_secs(u64::MAX), async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(32);

            let mut watcher = match RecommendedWatcher::new(
                move |res| {
                    let _ = tx.blocking_send(res);
                },
                Config::default().with_poll_interval(Duration::from_secs(2)),
            ) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[context-editor] watcher init error: {e}");
                    return;
                }
            };

            for sub in ["skills", "agents", "rules"] {
                let p = root.join(sub);
                if p.exists() {
                    let _ = watcher.watch(&p, RecursiveMode::Recursive);
                }
            }
            let agents_md = root.join("AGENTS.md");
            if agents_md.exists() {
                let _ = watcher.watch(&agents_md, RecursiveMode::NonRecursive);
            }

            eprintln!("[context-editor] file watcher active");

            while let Some(ev) = rx.recv().await {
                if ev.is_ok() {
                    let loaded = ContextStore::load_from(&root);
                    eprintln!(
                        "[context-editor] hot-reload: {} skills, {} agents, {} rules",
                        loaded.skills.len(),
                        loaded.agents.len(),
                        loaded.rules.len()
                    );
                    *store.write().unwrap() = loaded;
                }
            }
        });
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExtensionKind;
    use std::fs;

    fn make_context() -> CoreContext {
        use crate::{
            capability::{CapabilityRegistry, Capabilities},
            commands::CommandRegistry,
            context::InMemoryConfigStore,
            events::EventBus,
            permission::PermissionGate,
            version::VersionManager,
        };
        let perms = Arc::new(PermissionGate::new());
        CoreContext {
            bus: Arc::new(EventBus::new()),
            commands: Arc::new(CommandRegistry::new(perms.clone())),
            versions: Arc::new(VersionManager::new()),
            permissions: perms,
            caps: Arc::new(Capabilities::new()),
            capability_registry: Arc::new(CapabilityRegistry::new()),
            runtime: Arc::new(crate::runtime::Runtime::new()),
            config: Arc::new(InMemoryConfigStore::new()),
        }
    }

    fn make_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: "context-editor".into(),
            version: semver::Version::parse("0.1.0").unwrap(),
            kind: ExtensionKind::Panel,
            api_version: "v1".into(),
            schema_version: semver::Version::parse("1.0.0").unwrap(),
            min_core: semver::Version::parse("0.1.0").unwrap(),
            requires: vec![],
            optional: vec![],
            provides: vec![],
            commands: vec![],
            publishes: vec![],
            subscribes: vec![],
            fs_scopes: vec![],
            http_hosts: vec![],
        }
    }

    fn tmp_workspace() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "nulqor-ctx-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn parse_skill_roundtrip() {
        let content = r#"---
name: my-skill
description: does cool things
triggers: [foo, bar]
---

# My Skill

This is the body.
"#;
        let meta = parse_skill_frontmatter(content).expect("should parse");
        assert_eq!(meta.name, "my-skill");
        assert_eq!(meta.description, "does cool things");
        assert_eq!(meta.triggers, vec!["foo", "bar"]);
        assert!(meta.body.contains("This is the body."));
    }

    #[test]
    fn context_store_loads_skill_from_disk() {
        let root = tmp_workspace();
        let skill_dir = root.join("skills/test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: a test\ntriggers: []\n---\n\nBody here.",
        )
        .unwrap();

        let store = ContextStore::load_from(&root);
        assert_eq!(store.skills.len(), 1);
        assert_eq!(store.skills[0].name, "test-skill");
    }

    #[test]
    fn system_prompt_assembles_in_order() {
        let root = tmp_workspace();
        fs::write(root.join("AGENTS.md"), "You are Nulqor.").unwrap();
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::write(root.join("rules/01-rule.md"), "Always be helpful.").unwrap();
        fs::create_dir_all(root.join("skills/test-skill")).unwrap();
        fs::write(
            root.join("skills/test-skill/SKILL.md"),
            "---\nname: test-skill\ndescription: test\ntriggers: []\n---\n\nBody.",
        )
        .unwrap();

        let store = ContextStore::load_from(&root);
        let global = crate::workbench_prefs::WorkbenchPrefs::default();
        let prompt = store.assemble_system_prompt(None, &ContextPreferences::default(), &global);
        let agent_pos = prompt.find("You are Nulqor.").unwrap();
        let rule_pos = prompt.find("Always be helpful.").unwrap();
        let skill_pos = prompt.find("test-skill").unwrap();
        assert!(agent_pos < rule_pos, "agent should come before rules");
        assert!(rule_pos < skill_pos, "rules should come before skill index");
    }

    #[test]
    fn interpolate_date_replaces_current_datetime() {
        let result = interpolate_date("Now: {{current_datetime}}");
        assert!(!result.contains("{{current_datetime}}"));
        assert!(result.contains("UTC") || result.contains('-'));
    }

    #[test]
    fn system_prompt_includes_resolved_date_rule() {
        let root = tmp_workspace();
        fs::write(root.join("AGENTS.md"), "You are Nulqor.").unwrap();
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::write(
            root.join("rules/current-date.md"),
            "Current date and time: {{current_datetime}}\n",
        )
        .unwrap();

        let store = ContextStore::load_from(&root);
        let global = crate::workbench_prefs::WorkbenchPrefs::default();
        let prompt = store.assemble_system_prompt(None, &ContextPreferences::default(), &global);
        assert!(!prompt.contains("{{current_datetime}}"));
        assert!(prompt.contains("Current date and time:"));
    }

    #[test]
    fn system_prompt_respects_disabled_rules_and_skills() {
        let root = tmp_workspace();
        fs::write(root.join("AGENTS.md"), "You are Nulqor.").unwrap();
        fs::create_dir_all(root.join("rules")).unwrap();
        fs::write(root.join("rules/a.md"), "Rule A").unwrap();
        fs::write(root.join("rules/b.md"), "Rule B").unwrap();
        fs::create_dir_all(root.join("skills/s1")).unwrap();
        fs::write(
            root.join("skills/s1/SKILL.md"),
            "---\nname: s1\ndescription: one\ntriggers: []\n---\n\nBody.",
        )
        .unwrap();
        fs::create_dir_all(root.join("skills/s2")).unwrap();
        fs::write(
            root.join("skills/s2/SKILL.md"),
            "---\nname: s2\ndescription: two\ntriggers: []\n---\n\nBody.",
        )
        .unwrap();

        let store = ContextStore::load_from(&root);
        let mut prefs = ContextPreferences::default();
        prefs.set_rule_enabled("a.md", false);
        prefs.set_skill_enabled("s2", false);
        let global = crate::workbench_prefs::WorkbenchPrefs::default();
        let prompt = store.assemble_system_prompt(None, &prefs, &global);
        assert!(!prompt.contains("Rule A"));
        assert!(prompt.contains("Rule B"));
        assert!(prompt.contains("s1"));
        assert!(!prompt.contains("s2"));
    }

    #[test]
    fn system_prompt_respects_disabled_agent() {
        let root = tmp_workspace();
        fs::write(root.join("AGENTS.md"), "You are Nulqor.").unwrap();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(root.join("agents/reviewer.md"), "You are a reviewer.").unwrap();

        let store = ContextStore::load_from(&root);
        let mut prefs = ContextPreferences::default();
        prefs.set_agent_enabled("default", false);
        let global = crate::workbench_prefs::WorkbenchPrefs::default();
        let prompt = store.assemble_system_prompt(None, &prefs, &global);
        assert!(!prompt.contains("You are Nulqor."));
    }

    #[test]
    fn session_preferences_roundtrip() {
        let root = tmp_workspace();
        std::fs::create_dir_all(root.join(".nulqor/sessions")).unwrap();
        let mut prefs = ContextPreferences::default();
        prefs.set_rule_enabled("a.md", false);
        save_session_preferences(&root, "2026-05-24-test", &prefs).unwrap();
        let loaded = load_session_preferences(&root, "2026-05-24-test");
        assert!(!loaded.rule_enabled("a.md"));
    }

    #[test]
    fn context_editor_registers_commands() {
        let ctx = make_context();
        let ext = ContextEditorExtension::new(make_manifest());
        ext.activate(&ctx).expect("activate");

        let cmds = ctx.commands.list_commands();
        assert!(cmds.iter().any(|c| c == "context-editor:reload@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:list-skills@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:list-agents@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:list-rules@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:load-skill@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:load-rule@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:load-agent@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:save-skill@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:save-rule@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:save-agent@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:context-profile@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:set-context-profile@1"));
        assert!(cmds.iter().any(|c| c == "context-editor:system-prompt@1"));
    }
}
