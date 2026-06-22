//! Persistence extension — Phase 4.1 (BUILD_PLAN §4.1).
//!
//! Provides slotted capability `storage`/`main` (instance proof required by Phase 4 gate).
//!
//! Maintains a SQLite + FTS5 index over `.nulqor/sessions/*.jsonl`.
//! The JSONL files remain the truth store; SQLite is an indexer only.
//!
//! Schema:
//!   messages(id, session_id, role, content, timestamp, participant_name, driver, model)
//!   messages_fts  — FTS5 over content + participant_name (external content table)
//!
//! Commands:
//!   - `storage:search@1`        — FTS5 full-text search across all indexed messages.
//!   - `storage:index-session@1` — index a single session file by id.
//!   - `storage:reindex@1`       — reindex all session JSONL files under .nulqor/sessions/.
//!   - `storage:status@1`        — database stats and health.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OpenFlags, params};

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::types::{
    CapabilityDecl, CommandDecl, CommandId, EventPattern, ExtensionManifest, NamespacedEvent,
    Permission,
};

// ---------------------------------------------------------------------------
// SqliteStore — the capability handle
// ---------------------------------------------------------------------------

pub struct SqliteStore {
    conn: Mutex<Connection>,
    db_path: PathBuf,
}

impl SqliteStore {
    fn open(db_path: PathBuf) -> Result<Arc<Self>, CoreError> {
        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| CoreError::Io(format!("cannot open SQLite at {}: {e}", db_path.display())))?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous  = NORMAL;
             PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS messages (
                 id               TEXT NOT NULL,
                 session_id       TEXT NOT NULL,
                 role             TEXT NOT NULL,
                 content          TEXT NOT NULL,
                 timestamp        TEXT,
                 participant_name TEXT,
                 driver           TEXT,
                 model            TEXT,
                 PRIMARY KEY (session_id, id)
             );

             CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                 content,
                 participant_name,
                 content='messages',
                 content_rowid='rowid'
             );",
        )
        .map_err(|e| CoreError::Io(format!("cannot initialise SQLite schema: {e}")))?;

        Ok(Arc::new(Self {
            conn: Mutex::new(conn),
            db_path,
        }))
    }

    fn upsert_message(&self, session_id: &str, msg: &serde_json::Value) -> Result<(), CoreError> {
        let id = msg["id"].as_str().unwrap_or("").to_owned();
        if id.is_empty() { return Ok(()); }
        let role = msg["role"].as_str().unwrap_or("").to_owned();
        let content = msg["content"].as_str().unwrap_or("").to_owned();
        let timestamp = msg["timestamp"].as_str().map(str::to_owned);
        let participant = msg["participant_name"].as_str().map(str::to_owned);
        let driver = msg["driver"].as_str().map(str::to_owned);
        let model = msg["model"].as_str().map(str::to_owned);

        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT OR REPLACE INTO messages
                (id, session_id, role, content, timestamp, participant_name, driver, model)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, session_id, role, content, timestamp, participant, driver, model],
        )
        .map_err(|e| CoreError::Io(format!("upsert failed: {e}")))?;

        // Rebuild FTS for this row.
        conn.execute(
            "INSERT INTO messages_fts(messages_fts) VALUES('rebuild')",
            [],
        )
        .map_err(|e| CoreError::Io(format!("FTS rebuild failed: {e}")))?;

        Ok(())
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<serde_json::Value>, CoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, m.timestamp, m.participant_name, m.driver
             FROM messages_fts f
             JOIN messages m ON m.rowid = f.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )
        .map_err(|e| CoreError::Io(format!("search prepare failed: {e}")))?;

        let rows = stmt
            .query_map(params![query, limit as i64], |row| {
                Ok(serde_json::json!({
                    "id":               row.get::<_, String>(0)?,
                    "session_id":       row.get::<_, String>(1)?,
                    "role":             row.get::<_, String>(2)?,
                    "content":          row.get::<_, String>(3)?,
                    "timestamp":        row.get::<_, Option<String>>(4)?,
                    "participant_name": row.get::<_, Option<String>>(5)?,
                    "driver":           row.get::<_, Option<String>>(6)?,
                }))
            })
            .map_err(|e| CoreError::Io(format!("search query failed: {e}")))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| CoreError::Io(format!("row error: {e}")))?);
        }
        Ok(results)
    }

    fn index_session_file(&self, session_id: &str, path: &PathBuf) -> Result<usize, CoreError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CoreError::Io(format!("cannot read {}: {e}", path.display())))?;

        let mut count = 0;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            let msg: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| CoreError::Io(format!("bad JSONL line: {e}")))?;
            self.upsert_message(session_id, &msg)?;
            count += 1;
        }
        Ok(count)
    }

    fn message_count(&self) -> i64 {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
            .unwrap_or(0)
    }

    fn session_count(&self) -> i64 {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT COUNT(DISTINCT session_id) FROM messages",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

pub struct PersistenceExtension {
    #[allow(dead_code)]
    manifest: ExtensionManifest,
}

impl PersistenceExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for PersistenceExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let root = resolve_workspace_root();
        let db_dir = root.join(".nulqor");
        std::fs::create_dir_all(&db_dir)
            .map_err(|e| CoreError::Io(format!("cannot create .nulqor/: {e}")))?;
        let db_path = db_dir.join("index.db");

        let store = SqliteStore::open(db_path)?;

        // Register as storage/main capability.
        ctx.capability_registry.provide(
            "persistence",
            CapabilityDecl {
                capability: "storage".into(),
                instance: "main".into(),
                contract: "storage@1".into(),
            },
            store.clone(),
        )?;

        subscribe_message_added(&ctx.bus, store.clone());
        register_search(&ctx.commands, store.clone())?;
        register_index_session(&ctx.commands, store.clone(), root.clone())?;
        register_reindex(&ctx.commands, store.clone(), root.clone())?;
        register_status(&ctx.commands, store.clone())?;

        // If the index is empty on startup, reindex existing session files so
        // search works immediately without needing to call storage:reindex@1 manually.
        startup_reindex(&store, &root);

        eprintln!("[persistence] activated — storage/main @ .nulqor/index.db");
        Ok(())
    }
}

fn startup_reindex(store: &Arc<SqliteStore>, root: &std::path::Path) {
    if store.message_count() > 0 {
        return;
    }
    let sessions_dir = root.join(".nulqor").join("sessions");
    if !sessions_dir.exists() {
        return;
    }
    let entries = match std::fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let mut total_sessions = 0usize;
    let mut total_messages = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let session_id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_owned();
        if session_id.is_empty() {
            continue;
        }
        match store.index_session_file(&session_id, &path) {
            Ok(n) => {
                total_sessions += 1;
                total_messages += n;
            }
            Err(e) => {
                eprintln!("[persistence] startup reindex skipping {session_id}: {e}");
            }
        }
    }
    if total_sessions > 0 {
        eprintln!(
            "[persistence] startup reindex: {total_sessions} sessions, {total_messages} messages"
        );
    }
}

// ---------------------------------------------------------------------------
// Subscriptions
// ---------------------------------------------------------------------------

fn subscribe_message_added(bus: &Arc<EventBus>, store: Arc<SqliteStore>) {
    bus.subscribe(
        EventPattern::exact("transcript", "message-added", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            if let Some(msg) = ev.payload.get("message") {
                // Try to get the session_id from the payload; fall back to "active".
                let session_id = ev.payload["session_id"]
                    .as_str()
                    .unwrap_or("active")
                    .to_owned();
                if let Err(e) = store.upsert_message(&session_id, msg) {
                    eprintln!("[persistence] index error: {e}");
                }
            }
        }),
    );
}

// ---------------------------------------------------------------------------
// storage:search@1
// ---------------------------------------------------------------------------

fn register_search(
    registry: &Arc<crate::commands::CommandRegistry>,
    store: Arc<SqliteStore>,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "storage".into(),
                action: "search".into(),
                version: 1,
            },
            owner: "persistence".into(),
            input_schema: r#"{ "query": "string", "limit"?: "integer" }"#.into(),
            output_schema: r#"{ "results": "array", "total": "integer" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let query = input["query"]
                .as_str()
                .ok_or_else(|| CoreError::Io("storage:search requires 'query'".into()))?
                .to_owned();
            let limit = input["limit"].as_u64().map(|n| n as usize).unwrap_or(20);
            let results = store.search(&query, limit)?;
            let total = results.len();
            Ok(serde_json::json!({ "results": results, "total": total }))
        }),
    )
}

// ---------------------------------------------------------------------------
// storage:index-session@1
// ---------------------------------------------------------------------------

fn register_index_session(
    registry: &Arc<crate::commands::CommandRegistry>,
    store: Arc<SqliteStore>,
    root: PathBuf,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "storage".into(),
                action: "index-session".into(),
                version: 1,
            },
            owner: "persistence".into(),
            input_schema: r#"{ "session_id": "string" }"#.into(),
            output_schema: r#"{ "indexed": "integer", "session_id": "string" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("storage:index-session requires 'session_id'".into()))?
                .to_owned();
            let path = root.join(".nulqor").join("sessions").join(format!("{session_id}.jsonl"));
            if !path.exists() {
                return Err(CoreError::Io(format!("session file not found: {}", path.display())));
            }
            let indexed = store.index_session_file(&session_id, &path)?;
            eprintln!("[persistence] indexed {indexed} messages from session {session_id}");
            Ok(serde_json::json!({ "indexed": indexed, "session_id": session_id }))
        }),
    )
}

// ---------------------------------------------------------------------------
// storage:reindex@1
// ---------------------------------------------------------------------------

fn register_reindex(
    registry: &Arc<crate::commands::CommandRegistry>,
    store: Arc<SqliteStore>,
    root: PathBuf,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "storage".into(),
                action: "reindex".into(),
                version: 1,
            },
            owner: "persistence".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "sessions": "integer", "messages": "integer" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |_input| {
            let sessions_dir = root.join(".nulqor").join("sessions");
            if !sessions_dir.exists() {
                return Ok(serde_json::json!({ "sessions": 0, "messages": 0 }));
            }
            let mut total_sessions = 0;
            let mut total_messages = 0;
            let entries = std::fs::read_dir(&sessions_dir)
                .map_err(|e| CoreError::Io(format!("cannot read sessions/: {e}")))?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let session_id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_owned();
                if session_id.is_empty() { continue; }
                match store.index_session_file(&session_id, &path) {
                    Ok(n) => {
                        total_sessions += 1;
                        total_messages += n;
                    }
                    Err(e) => {
                        eprintln!("[persistence] skipping {session_id}: {e}");
                    }
                }
            }
            eprintln!("[persistence] reindex complete: {total_sessions} sessions, {total_messages} messages");
            Ok(serde_json::json!({ "sessions": total_sessions, "messages": total_messages }))
        }),
    )
}

// ---------------------------------------------------------------------------
// storage:status@1
// ---------------------------------------------------------------------------

fn register_status(
    registry: &Arc<crate::commands::CommandRegistry>,
    store: Arc<SqliteStore>,
) -> Result<(), CoreError> {
    registry.register(
        CommandDecl {
            id: CommandId {
                namespace: "storage".into(),
                action: "status".into(),
                version: 1,
            },
            owner: "persistence".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "db_path": "string", "message_count": "integer", "session_count": "integer", "ok": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_input| {
            Ok(serde_json::json!({
                "db_path": store.db_path.to_string_lossy(),
                "message_count": store.message_count(),
                "session_count": store.session_count(),
                "ok": true,
            }))
        }),
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

    fn tmp_db() -> Arc<SqliteStore> {
        let dir = std::env::temp_dir().join(format!(
            "nulqor-persistence-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        SqliteStore::open(dir.join("test.db")).unwrap()
    }

    #[test]
    fn open_creates_schema() {
        let store = tmp_db();
        assert_eq!(store.message_count(), 0);
        assert_eq!(store.session_count(), 0);
    }

    #[test]
    fn upsert_and_count() {
        let store = tmp_db();
        let msg = serde_json::json!({
            "id": "msg-1",
            "role": "user",
            "content": "Hello world",
            "timestamp": "2026-06-19T00:00:00Z",
            "participant_name": "Human",
            "driver": "human",
        });
        store.upsert_message("session-a", &msg).unwrap();
        assert_eq!(store.message_count(), 1);
    }

    #[test]
    fn upsert_is_idempotent() {
        let store = tmp_db();
        let msg = serde_json::json!({ "id": "m1", "role": "user", "content": "hi", "participant_name": null, "driver": null });
        store.upsert_message("s1", &msg).unwrap();
        store.upsert_message("s1", &msg).unwrap();
        assert_eq!(store.message_count(), 1);
    }

    #[test]
    fn search_finds_content() {
        let store = tmp_db();
        let msg = serde_json::json!({
            "id": "m-search",
            "role": "assistant",
            "content": "The quick brown fox jumps over the lazy dog",
            "participant_name": "Model",
            "driver": "assistant",
        });
        store.upsert_message("session-search", &msg).unwrap();
        let results = store.search("fox", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["id"], "m-search");
    }

    #[test]
    fn index_session_file() {
        let store = tmp_db();
        let dir = std::env::temp_dir().join(format!(
            "nulqor-persistence-sessions-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        let path = dir.join("test-session.jsonl");
        let lines = vec![
            r#"{"id":"m1","role":"user","content":"First message","participant_name":"Human","driver":"human"}"#,
            r#"{"id":"m2","role":"assistant","content":"Second message","participant_name":"Model","driver":"assistant"}"#,
        ];
        std::fs::write(&path, lines.join("\n")).unwrap();

        let count = store.index_session_file("test-session", &path).unwrap();
        assert_eq!(count, 2);
        assert_eq!(store.message_count(), 2);
    }

    #[test]
    fn multiple_sessions_counted() {
        let store = tmp_db();
        for (sid, mid, content) in [
            ("s1", "m1", "alpha"),
            ("s1", "m2", "beta"),
            ("s2", "m3", "gamma"),
        ] {
            store.upsert_message(sid, &serde_json::json!({
                "id": mid, "role": "user", "content": content,
                "participant_name": null, "driver": null
            })).unwrap();
        }
        assert_eq!(store.message_count(), 3);
        assert_eq!(store.session_count(), 2);
    }
}
