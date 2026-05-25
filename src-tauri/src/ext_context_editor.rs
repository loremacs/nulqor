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

use serde::Deserialize;

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

impl ContextStore {
    fn load_from(root: &Path) -> Self {
        let mut store = ContextStore::default();

        // Skills: skills/<name>/SKILL.md with YAML frontmatter
        let skills_dir = root.join("skills");
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let skill_md = entry.path().join("SKILL.md");
                if skill_md.exists() {
                    if let Ok(content) = std::fs::read_to_string(&skill_md) {
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
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    matches!(ext, "md" | "mdc" | "txt") && stem != "INDEX"
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

    /// Assemble system prompt per decisions/006 §6.
    pub fn assemble_system_prompt(&self, agent: Option<&str>) -> String {
        let agent_key = agent.unwrap_or("default");
        let mut parts: Vec<String> = Vec::new();

        // 1. Agent persona
        if let Some(a) = self.agents.get(agent_key).or_else(|| self.agents.get("default")) {
            parts.push(interpolate_date(&a.body));
        }

        // 2. Rules (concatenated in alphabetical order, date placeholders resolved)
        if !self.rules.is_empty() {
            parts.push(
                self.rules
                    .iter()
                    .map(|r| interpolate_date(&r.body))
                    .collect::<Vec<_>>()
                    .join("\n\n"),
            );
        }

        // 3. Compact skill index
        if !self.skills.is_empty() {
            let index = self
                .skills
                .iter()
                .map(|s| format!("- **{}**: {}", s.name, s.description))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("## Available Skills\n{index}"));
        }

        parts.join("\n\n---\n\n")
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

    let front: Front = serde_yaml::from_str(front_yaml).ok()?;
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
        Self { manifest, store: Arc::new(RwLock::new(ContextStore::default())) }
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

        self.register_commands(ctx)?;
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
    fn register_commands(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let store = self.store.clone();

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
                    let agents = s
                        .read()
                        .unwrap()
                        .agents
                        .keys()
                        .map(|k| serde_json::json!({ "name": k }))
                        .collect::<Vec<_>>();
                    Ok(serde_json::json!({ "agents": agents }))
                }),
            )?;
        }

        // list-rules
        {
            let s = store.clone();
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
                    let rules = s
                        .read()
                        .unwrap()
                        .rules
                        .iter()
                        .map(|r| {
                            serde_json::json!({
                                "filename": r.filename,
                                "excerpt": r.excerpt,
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

        // system-prompt
        {
            let s = store.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "context-editor".into(),
                        action: "system-prompt".into(),
                        version: 1,
                    },
                    owner: "context-editor".into(),
                    input_schema: r#"{ "agent": "string?" }"#.into(),
                    output_schema: r#"{ "prompt": "string", "token_estimate": "number" }"#.into(),
                    callable_by: vec!["panel".into(), "agent".into(), "service".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let agent = input["agent"].as_str().map(str::to_owned);
                    let prompt = s.read().unwrap().assemble_system_prompt(agent.as_deref());
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
        let prompt = store.assemble_system_prompt(None);
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
        let prompt = store.assemble_system_prompt(None);
        assert!(!prompt.contains("{{current_datetime}}"));
        assert!(prompt.contains("Current date and time:"));
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
        assert!(cmds.iter().any(|c| c == "context-editor:system-prompt@1"));
    }
}
