//! Decision-records extension — Phase 4.5 (BUILD_PLAN §4.5).
//!
//! Captures architectural decisions as `docs/decisions/<NNN>-<slug>.md`.
//! Auto-numbers from existing files so the builder never has to count by hand.
//!
//! Commands:
//!   - `decisions:create@1` — write a new ADR; returns path + number.
//!   - `decisions:list@1`   — list existing decisions (number, title, status, path).

use std::path::PathBuf;
use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

pub struct DecisionRecordsExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl DecisionRecordsExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for DecisionRecordsExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        register_create(&ctx.commands)?;
        register_list(&ctx.commands)?;
        eprintln!("[decision-records] activated");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// decisions:create@1
// ---------------------------------------------------------------------------

fn register_create(registry: &Arc<crate::commands::CommandRegistry>) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "decisions".into(),
                action: "create".into(),
                version: 1,
            },
            owner: "decision-records".into(),
            input_schema: r#"{ "title": "string", "context": "string", "decision": "string" }"#
                .into(),
            output_schema: r#"{ "path": "string", "number": "integer" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let title = input["title"]
                .as_str()
                .ok_or_else(|| CoreError::Io("decisions:create requires 'title'".into()))?
                .to_owned();
            let ctx_text = input["context"]
                .as_str()
                .ok_or_else(|| CoreError::Io("decisions:create requires 'context'".into()))?
                .to_owned();
            let decision_text = input["decision"]
                .as_str()
                .ok_or_else(|| CoreError::Io("decisions:create requires 'decision'".into()))?
                .to_owned();
            let status = input["status"].as_str().unwrap_or("Accepted").to_owned();
            let supersedes = input["supersedes"].as_str().map(|s| s.to_owned());
            let consequences = input["consequences"]
                .as_str()
                .unwrap_or("(to be documented as they emerge)")
                .to_owned();

            let root = resolve_workspace_root();
            let decisions_dir = root.join("docs").join("decisions");
            std::fs::create_dir_all(&decisions_dir)
                .map_err(|e| CoreError::Io(format!("cannot create decisions dir: {e}")))?;

            let next_num = next_decision_number(&decisions_dir);
            let slug = slugify(&title);
            let filename = format!("{:03}-{}.md", next_num, slug);
            let path = decisions_dir.join(&filename);

            let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
            let supersedes_line = match &supersedes {
                Some(s) => format!("**Supersedes:** {}\n", s),
                None => String::new(),
            };

            let body = format!(
                "# {:03} — {}\n\n**Status:** {}  \n**Date:** {}  \n{}\n## Context\n\n{}\n\n## Decision\n\n{}\n\n## Consequences\n\n{}\n",
                next_num, title, status, date, supersedes_line, ctx_text, decision_text, consequences
            );

            std::fs::write(&path, &body)
                .map_err(|e| CoreError::Io(format!("cannot write decision file: {e}")))?;

            let rel_path = format!("docs/decisions/{}", filename);
            eprintln!("[decision-records] created {}", rel_path);

            Ok(serde_json::json!({
                "path": rel_path,
                "number": next_num,
            }))
        }),
    )
}

// ---------------------------------------------------------------------------
// decisions:list@1
// ---------------------------------------------------------------------------

fn register_list(registry: &Arc<crate::commands::CommandRegistry>) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "decisions".into(),
                action: "list".into(),
                version: 1,
            },
            owner: "decision-records".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "decisions": "array" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_input| {
            let root = resolve_workspace_root();
            let decisions_dir = root.join("docs").join("decisions");
            let entries = list_decisions(&decisions_dir);
            Ok(serde_json::json!({ "decisions": entries }))
        }),
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn next_decision_number(dir: &PathBuf) -> u32 {
    let max = read_existing_numbers(dir).into_iter().max().unwrap_or(0);
    max + 1
}

fn read_existing_numbers(dir: &PathBuf) -> Vec<u32> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if !name.ends_with(".md") { return None; }
            // Expect NNN-... format
            let prefix = name.split('-').next()?;
            prefix.parse::<u32>().ok()
        })
        .collect()
}

fn list_decisions(dir: &PathBuf) -> Vec<serde_json::Value> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut items: Vec<(u32, String, serde_json::Value)> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            if !name.ends_with(".md") { return None; }
            let num: u32 = name.split('-').next()?.parse().ok()?;
            let path = e.path();
            let content = std::fs::read_to_string(&path).ok()?;
            let title = parse_title(&content).unwrap_or_else(|| name.clone());
            let status = parse_status(&content).unwrap_or_else(|| "Unknown".into());
            let rel = format!("docs/decisions/{}", name);
            Some((num, name.clone(), serde_json::json!({
                "number": num,
                "title": title,
                "status": status,
                "path": rel,
            })))
        })
        .collect();
    items.sort_by_key(|(n, _, _)| *n);
    items.into_iter().map(|(_, _, v)| v).collect()
}

fn parse_title(content: &str) -> Option<String> {
    content.lines().find(|l| l.starts_with("# ")).map(|l| l.trim_start_matches("# ").to_owned())
}

fn parse_status(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("**Status:**") {
            return Some(rest.trim().to_owned());
        }
    }
    None
}

fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
        .chars()
        .take(50)
        .collect()
}

fn resolve_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some(parent) = cwd.parent() {
        if parent.join("extensions").exists() || parent.join("AGENTS.md").exists() {
            return parent.to_path_buf();
        }
    }
    cwd
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "nulqor-decisions-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(slugify("SQLite + FTS5 indexer"), "sqlite-fts5-indexer");
        assert_eq!(slugify("  multiple   spaces  "), "multiple-spaces");
    }

    #[test]
    fn next_decision_number_empty_dir() {
        let dir = tmp_dir();
        assert_eq!(next_decision_number(&dir), 1);
    }

    #[test]
    fn next_decision_number_with_existing() {
        let dir = tmp_dir();
        std::fs::write(dir.join("001-frozen-core.md"), "# 001 — Frozen Core").unwrap();
        std::fs::write(dir.join("009-sessions.md"), "# 009 — Sessions").unwrap();
        assert_eq!(next_decision_number(&dir), 10);
    }

    #[test]
    fn list_decisions_parses_title_and_status() {
        let dir = tmp_dir();
        std::fs::write(
            dir.join("001-test.md"),
            "# 001 — Test Decision\n\n**Status:** Accepted  \n**Date:** 2026-01-01\n",
        )
        .unwrap();
        let items = list_decisions(&dir);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "001 — Test Decision");
        assert_eq!(items[0]["status"], "Accepted");
        assert_eq!(items[0]["number"], 1);
    }

    #[test]
    fn write_decision_file() {
        let dir = tmp_dir();
        let docs = dir.join("docs").join("decisions");
        std::fs::create_dir_all(&docs).unwrap();

        let num = next_decision_number(&docs);
        let slug = slugify("Context manager token budget");
        let filename = format!("{:03}-{}.md", num, slug);
        let body = format!("# {:03} — Context manager token budget\n\n**Status:** Draft\n", num);
        std::fs::write(docs.join(&filename), &body).unwrap();

        let items = list_decisions(&docs);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["status"], "Draft");
    }
}
