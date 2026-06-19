//! Run-logger extension — Phase 3.4 (BUILD_PLAN §3.4).
//!
//! Appends every turn to `runs/YYYY-MM-DD.jsonl` (one JSON line per message).
//! Subscribes to `transcript:message-added@1`.
//!
//! Each JSONL line records: ts, role, content, model, tokens, latency_ms,
//! driver, participant_name, and optional reasoning — enough to compare
//! before/after across runs and prove the compounding loop is real.

use std::io::Write as IoWrite;
use std::path::PathBuf;
use std::sync::Arc;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{EventPattern, ExtensionManifest};

pub struct RunLoggerExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl RunLoggerExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for RunLoggerExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let root = resolve_workspace_root();

        ctx.bus.subscribe(
            EventPattern::exact("transcript", "message-added", 1),
            Arc::new(move |event: &crate::types::NamespacedEvent| {
                if let Some(msg) = event.payload.get("message") {
                    if let Err(e) = append_run_entry(&root, msg) {
                        eprintln!("[run-logger] write error: {e}");
                    }
                }
            }),
        );

        eprintln!("[run-logger] activated — writing to runs/YYYY-MM-DD.jsonl");
        Ok(())
    }
}

fn append_run_entry(root: &PathBuf, msg: &serde_json::Value) -> std::io::Result<()> {
    let runs_dir = root.join("runs");
    std::fs::create_dir_all(&runs_dir)?;

    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let log_path = runs_dir.join(format!("{date}.jsonl"));

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let line = serde_json::to_string(msg).unwrap_or_else(|err| {
        eprintln!("[run-logger] serialization failed: {:?}", err);
        "{}".into()
    });
    writeln!(file, "{line}")?;
    Ok(())
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

    #[test]
    fn append_run_entry_creates_file() {
        let root = std::env::temp_dir().join(format!(
            "nulqor-run-logger-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let msg = serde_json::json!({
            "id": "msg-1",
            "role": "assistant",
            "content": "Hello",
            "model": "gemma-4b",
            "tokens": 5,
            "latency_ms": 120,
            "driver": "subject",
            "participant_name": "Subject",
        });

        append_run_entry(&root, &msg).unwrap();

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let path = root.join("runs").join(format!("{date}.jsonl"));
        assert!(path.exists(), "log file should exist");

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("Hello"), "log should contain message content");
    }

    #[test]
    fn append_run_entry_appends_multiple() {
        let root = std::env::temp_dir().join(format!(
            "nulqor-run-logger-multi-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let msg = serde_json::json!({ "role": "user", "content": "turn1" });
        append_run_entry(&root, &msg).unwrap();
        let msg2 = serde_json::json!({ "role": "assistant", "content": "turn2" });
        append_run_entry(&root, &msg2).unwrap();

        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let path = root.join("runs").join(format!("{date}.jsonl"));
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<_> = contents.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 2, "should have two JSONL lines");
    }
}
