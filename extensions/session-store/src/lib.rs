//! Session store — file-backed sessions, human rail, and archived forks.
//!
//! Agent contract: `.nulqor/sessions/<id>.jsonl` only.
//! Human-only: `.nulqor/human/**` (rails, branches, catalog).

use std::io::{BufRead, BufReader, Write as IoWrite};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{
    CommandDecl, CommandId, EventPattern, ExtensionManifest, Permission,
};

// ---------------------------------------------------------------------------
// On-disk shapes
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
struct StoreState {
    active_session_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CatalogEntry {
    id: String,
    title: String,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
    summary: String,
    mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct Catalog {
    sessions: Vec<CatalogEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RailMarker {
    id: String,
    kind: String,
    #[serde(rename = "type")]
    marker_type: String,
    message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fork_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symbol: Option<String>,
    preview: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branch_file: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct RailFile {
    session_id: String,
    markers: Vec<RailMarker>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ForkRecord {
    id: String,
    label: String,
    forked_at_message: String,
    file: String,
    created: DateTime<Utc>,
    message_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ForkIndex {
    session_id: String,
    forks: Vec<ForkRecord>,
}

#[derive(Clone)]
struct StorePaths {
    root: PathBuf,
    state: PathBuf,
    sessions: PathBuf,
    catalog: PathBuf,
    rails: PathBuf,
    branches: PathBuf,
}

impl StorePaths {
    fn new(root: PathBuf) -> Self {
        let base = root.join(".nulqor");
        Self {
            state: base.join("state.json"),
            sessions: base.join("sessions"),
            catalog: base.join("human").join("catalog.json"),
            rails: base.join("human").join("rails"),
            branches: base.join("human").join("branches"),
            root,
        }
    }

    fn session_file(&self, id: &str) -> PathBuf {
        self.sessions.join(format!("{id}.jsonl"))
    }

    fn session_context_file(&self, id: &str) -> PathBuf {
        self.sessions.join(format!("{id}.context.json"))
    }

    fn rail_file(&self, id: &str) -> PathBuf {
        self.rails.join(format!("{id}.json"))
    }

    fn branch_dir(&self, session_id: &str) -> PathBuf {
        self.branches.join(session_id)
    }

    fn fork_index_file(&self, session_id: &str) -> PathBuf {
        self.branch_dir(session_id).join("index.json")
    }

    fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.sessions)?;
        std::fs::create_dir_all(&self.rails)?;
        std::fs::create_dir_all(&self.branches)?;
        if let Some(human) = self.catalog.parent() {
            std::fs::create_dir_all(human)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

pub struct SessionStoreExtension {
    manifest: ExtensionManifest,
    paths: StorePaths,
    active_id: Arc<RwLock<String>>,
}

impl SessionStoreExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        let root = resolve_workspace_root();
        Self {
            manifest,
            paths: StorePaths::new(root),
            active_id: Arc::new(RwLock::new(String::new())),
        }
    }
}

impl Extension for SessionStoreExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        self.paths.ensure_dirs().map_err(io_err)?;

        let session_id = self.ensure_active_session()?;
        *self.active_id.write().map_err(|_| CoreError::Io("lock poisoned".into()))? = session_id.clone();

        let messages = read_session_messages(&self.paths, &session_id)?;
        rebuild_rail_from_messages(&self.paths, &session_id, &messages)?;
        hydrate_transcript(&ctx.commands, &messages)?;

        let paths = self.paths.clone();
        let active = self.active_id.clone();

        ctx.bus.subscribe(
            EventPattern::exact("transcript", "message-added", 1),
            Arc::new(move |ev| {
                let sid = active.read().unwrap().clone();
                if sid.is_empty() {
                    return;
                }
                let Some(msg) = ev.payload.get("message") else {
                    return;
                };
                if let Err(e) = on_message_added(&paths, &sid, msg) {
                    eprintln!("[session-store] persist error: {e}");
                }
            }),
        );

        let paths2 = self.paths.clone();
        let active2 = self.active_id.clone();
        ctx.bus.subscribe(
            EventPattern::exact("transcript", "hydrated", 1),
            Arc::new(move |ev| {
                let sid = active2.read().unwrap().clone();
                if sid.is_empty() {
                    return;
                }
                let Some(msgs) = ev.payload.get("messages").and_then(|m| m.as_array()) else {
                    return;
                };
                if let Err(e) = rewrite_session_file(&paths2, &sid, msgs) {
                    eprintln!("[session-store] rewrite after hydrate: {e}");
                }
            }),
        );

        register_list(self.paths.clone(), self.active_id.clone(), &ctx.commands)?;
        register_create(
            self.paths.clone(),
            self.active_id.clone(),
            ctx.commands.clone(),
        )?;
        register_load(
            self.paths.clone(),
            self.active_id.clone(),
            ctx.commands.clone(),
        )?;
        register_active(self.paths.clone(), self.active_id.clone(), &ctx.commands)?;
        register_update(self.paths.clone(), &ctx.commands)?;
        register_delete(
            self.paths.clone(),
            self.active_id.clone(),
            ctx.commands.clone(),
        )?;
        register_edit_message(
            self.paths.clone(),
            self.active_id.clone(),
            ctx.commands.clone(),
        )?;
        register_rail_list(self.paths.clone(), self.active_id.clone(), &ctx.commands)?;
        register_rail_add_marker(
            self.paths.clone(),
            self.active_id.clone(),
            &ctx.commands,
        )?;
        register_branch_list(self.paths.clone(), self.active_id.clone(), &ctx.commands)?;
        register_branch_open(self.paths.clone(), self.active_id.clone(), &ctx.commands)?;

        eprintln!("[session-store] active session: {session_id}");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Session lifecycle
// ---------------------------------------------------------------------------

impl SessionStoreExtension {
    fn ensure_active_session(&self) -> Result<String, CoreError> {
        if self.paths.state.exists() {
            let state: StoreState = read_json(&self.paths.state)?;
            if self.paths.session_file(&state.active_session_id).exists() {
                return Ok(state.active_session_id);
            }
        }

        let id = new_session_id();
        write_json(
            &self.paths.state,
            &StoreState {
                active_session_id: id.clone(),
            },
        )?;
        rewrite_session_file(&self.paths, &id, &[])?;
        init_rail(&self.paths, &id)?;
        upsert_catalog_entry(
            &self.paths,
            &id,
            "New session",
            "",
            Utc::now(),
            Utc::now(),
        )?;
        Ok(id)
    }
}

fn new_session_id() -> String {
    let stamp = Utc::now().format("%Y-%m-%d").to_string();
    let short = Uuid::new_v4().to_string().chars().take(8).collect::<String>();
    format!("{stamp}-{short}")
}

fn set_active_session(paths: &StorePaths, id: &str) -> Result<(), CoreError> {
    write_json(
        &paths.state,
        &StoreState {
            active_session_id: id.to_owned(),
        },
    )
}

fn read_session_messages(paths: &StorePaths, id: &str) -> Result<Vec<serde_json::Value>, CoreError> {
    let file = paths.session_file(id);
    if !file.exists() {
        return Ok(vec![]);
    }
    let f = std::fs::File::open(&file).map_err(io_err)?;
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(io_err)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(serde_json::from_str(trimmed).map_err(json_err)?);
    }
    Ok(out)
}

fn rewrite_session_file(
    paths: &StorePaths,
    id: &str,
    messages: &[serde_json::Value],
) -> Result<(), CoreError> {
    let file = paths.session_file(id);
    let mut f = std::fs::File::create(&file).map_err(io_err)?;
    for msg in messages {
        let line = serde_json::to_string(msg).map_err(json_err)?;
        writeln!(f, "{line}").map_err(io_err)?;
    }
    Ok(())
}

fn hydrate_transcript(
    cmds: &Arc<crate::commands::CommandRegistry>,
    messages: &[serde_json::Value],
) -> Result<(), CoreError> {
    cmds.invoke(
        "session-store",
        &CommandId::parse("transcript:hydrate@1").unwrap(),
        serde_json::json!({ "messages": messages }),
    )
    .map(|_| ())
}

fn on_message_added(
    paths: &StorePaths,
    session_id: &str,
    msg: &serde_json::Value,
) -> Result<(), CoreError> {
    let file = paths.session_file(session_id);
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file)
        .map_err(io_err)?;
    let line = serde_json::to_string(msg).map_err(json_err)?;
    writeln!(f, "{line}").map_err(io_err)?;

    let preview = message_preview(msg);
    let role = msg["role"].as_str().unwrap_or("user");
    let marker_type = if role == "assistant" {
        "assistant"
    } else {
        "human"
    };
    append_auto_marker(
        paths,
        session_id,
        marker_type,
        msg["id"].as_str().unwrap_or(""),
        &preview,
    )?;

    let title = catalog_title(session_id);
    let summary = if role == "user" {
        preview.clone()
    } else {
        String::new()
    };
    let now = Utc::now();
    upsert_catalog_entry(paths, session_id, &title, &summary, now, now)?;

    Ok(())
}

fn message_preview(msg: &serde_json::Value) -> String {
    let content = msg["content"].as_str().unwrap_or("");
    let one_line: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.len() <= 48 {
        one_line
    } else {
        format!("{}…", &one_line[..48])
    }
}

fn catalog_title(session_id: &str) -> String {
    session_id.replace('-', " ")
}

fn upsert_catalog_entry(
    paths: &StorePaths,
    id: &str,
    title: &str,
    summary: &str,
    created: DateTime<Utc>,
    updated: DateTime<Utc>,
) -> Result<(), CoreError> {
    let mut catalog: Catalog = if paths.catalog.exists() {
        read_json(&paths.catalog)?
    } else {
        Catalog::default()
    };

    if let Some(entry) = catalog.sessions.iter_mut().find(|e| e.id == id) {
        if !summary.is_empty() && entry.summary.is_empty() {
            entry.summary = summary.to_owned();
        }
        entry.updated = updated;
    } else {
        catalog.sessions.push(CatalogEntry {
            id: id.to_owned(),
            title: title.to_owned(),
            created,
            updated,
            summary: summary.to_owned(),
            mode: "thread".into(),
        });
    }

    catalog.sessions.sort_by(|a, b| b.updated.cmp(&a.updated));
    write_json(&paths.catalog, &catalog)
}

fn update_catalog_entry(
    paths: &StorePaths,
    id: &str,
    title: Option<&str>,
    summary: Option<&str>,
) -> Result<(), CoreError> {
    let mut catalog: Catalog = if paths.catalog.exists() {
        read_json(&paths.catalog)?
    } else {
        Catalog::default()
    };

    let entry = catalog
        .sessions
        .iter_mut()
        .find(|e| e.id == id)
        .ok_or_else(|| CoreError::Io(format!("session not found: {id}")))?;

    if let Some(t) = title {
        entry.title = t.to_owned();
    }
    if let Some(s) = summary {
        entry.summary = s.to_owned();
    }
    entry.updated = Utc::now();
    catalog.sessions.sort_by(|a, b| b.updated.cmp(&a.updated));
    write_json(&paths.catalog, &catalog)
}

fn delete_session_files(paths: &StorePaths, session_id: &str) -> Result<(), CoreError> {
    let session_path = paths.session_file(session_id);
    if session_path.exists() {
        std::fs::remove_file(&session_path).map_err(io_err)?;
    }
    let context_path = paths.session_context_file(session_id);
    if context_path.exists() {
        std::fs::remove_file(&context_path).map_err(io_err)?;
    }
    let rail_path = paths.rail_file(session_id);
    if rail_path.exists() {
        std::fs::remove_file(&rail_path).map_err(io_err)?;
    }
    let branch_path = paths.branch_dir(session_id);
    if branch_path.exists() {
        std::fs::remove_dir_all(&branch_path).map_err(io_err)?;
    }
    Ok(())
}

/// Ensure `.nulqor/sessions/<id>.jsonl` and rail stub exist for a catalog entry.
fn ensure_session_materialized(paths: &StorePaths, session_id: &str) -> Result<(), CoreError> {
    paths.ensure_dirs().map_err(io_err)?;
    if !paths.session_file(session_id).exists() {
        rewrite_session_file(paths, session_id, &[])?;
    }
    if !paths.rail_file(session_id).exists() {
        init_rail(paths, session_id)?;
    }
    Ok(())
}

fn session_has_artifacts(paths: &StorePaths, session_id: &str) -> bool {
    paths.session_file(session_id).exists()
        || paths.rail_file(session_id).exists()
        || paths.branch_dir(session_id).exists()
}

fn create_empty_session(
    paths: &StorePaths,
    active: &Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
    title: &str,
) -> Result<String, CoreError> {
    let id = new_session_id();
    rewrite_session_file(paths, &id, &[])?;
    init_rail(paths, &id)?;
    let now = Utc::now();
    upsert_catalog_entry(paths, &id, title, "", now, now)?;
    set_active_session(paths, &id)?;
    *active.write().map_err(|_| CoreError::Io("lock poisoned".into()))? = id.clone();
    hydrate_transcript(cmds, &[])?;
    Ok(id)
}

fn init_rail(paths: &StorePaths, session_id: &str) -> Result<(), CoreError> {
    write_json(
        &paths.rail_file(session_id),
        &RailFile {
            session_id: session_id.to_owned(),
            markers: vec![],
        },
    )
}

fn read_rail(paths: &StorePaths, session_id: &str) -> Result<RailFile, CoreError> {
    let file = paths.rail_file(session_id);
    if !file.exists() {
        return Ok(RailFile {
            session_id: session_id.to_owned(),
            markers: vec![],
        });
    }
    read_json(&file)
}

fn write_rail(paths: &StorePaths, rail: &RailFile) -> Result<(), CoreError> {
    write_json(&paths.rail_file(&rail.session_id), rail)
}

fn rebuild_rail_from_messages(
    paths: &StorePaths,
    session_id: &str,
    messages: &[serde_json::Value],
) -> Result<(), CoreError> {
    let mut rail = RailFile {
        session_id: session_id.to_owned(),
        markers: vec![],
    };
    for msg in messages {
        let role = msg["role"].as_str().unwrap_or("user");
        let marker_type = if role == "assistant" {
            "assistant"
        } else {
            "human"
        };
        rail.markers.push(RailMarker {
            id: Uuid::new_v4().to_string(),
            kind: "auto".into(),
            marker_type: marker_type.to_owned(),
            message_id: msg["id"].as_str().unwrap_or("").to_owned(),
            fork_id: None,
            symbol: None,
            preview: message_preview(msg),
            note: None,
            branch_file: None,
        });
    }
    if paths.rail_file(session_id).exists() {
        let existing = read_rail(paths, session_id)?;
        for m in existing.markers {
            if m.kind == "user" || m.marker_type == "fork" {
                rail.markers.push(m);
            }
        }
    }
    write_rail(paths, &rail)
}

fn append_auto_marker(
    paths: &StorePaths,
    session_id: &str,
    marker_type: &str,
    message_id: &str,
    preview: &str,
) -> Result<(), CoreError> {
    if message_id.is_empty() {
        return Ok(());
    }
    let mut rail = read_rail(paths, session_id)?;
    if rail
        .markers
        .iter()
        .any(|m| m.message_id == message_id && m.kind == "auto")
    {
        return Ok(());
    }
    rail.markers.push(RailMarker {
        id: Uuid::new_v4().to_string(),
        kind: "auto".into(),
        marker_type: marker_type.to_owned(),
        message_id: message_id.to_owned(),
        fork_id: None,
        symbol: None,
        preview: preview.to_owned(),
        note: None,
        branch_file: None,
    });
    write_rail(paths, &rail)
}

fn append_fork_marker(
    paths: &StorePaths,
    session_id: &str,
    message_id: &str,
    fork_id: &str,
    branch_file: &str,
    preview: &str,
    message_count: usize,
) -> Result<(), CoreError> {
    let mut rail = read_rail(paths, session_id)?;
    rail.markers.push(RailMarker {
        id: Uuid::new_v4().to_string(),
        kind: "auto".into(),
        marker_type: "fork".into(),
        message_id: message_id.to_owned(),
        fork_id: Some(fork_id.to_owned()),
        symbol: Some("fork".into()),
        preview: preview.to_owned(),
        note: Some(format!("{message_count} messages archived")),
        branch_file: Some(branch_file.to_owned()),
    });
    write_rail(paths, &rail)?;

    let index_path = paths.fork_index_file(session_id);
    let mut index: ForkIndex = if index_path.exists() {
        read_json(&index_path)?
    } else {
        ForkIndex {
            session_id: session_id.to_owned(),
            forks: vec![],
        }
    };
    index.forks.push(ForkRecord {
        id: fork_id.to_owned(),
        label: preview.to_owned(),
        forked_at_message: message_id.to_owned(),
        file: branch_file.to_owned(),
        created: Utc::now(),
        message_count,
    });
    std::fs::create_dir_all(paths.branch_dir(session_id)).map_err(io_err)?;
    write_json(&index_path, &index)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn register_list(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "list".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "sessions": "array", "active_session_id": "string" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            paths.ensure_dirs().map_err(io_err)?;
            let catalog: Catalog = if paths.catalog.exists() {
                read_json(&paths.catalog)?
            } else {
                Catalog::default()
            };
            for entry in &catalog.sessions {
                ensure_session_materialized(&paths, &entry.id)?;
            }
            Ok(serde_json::json!({
                "sessions": catalog.sessions,
                "active_session_id": active.read().unwrap().clone(),
            }))
        }),
    )
}

fn register_create(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "create".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "title": "string?" }"#.into(),
            output_schema: r#"{ "id": "string" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            paths.ensure_dirs().map_err(io_err)?;
            let id = new_session_id();
            let title = input["title"]
                .as_str()
                .unwrap_or("New session")
                .to_owned();
            ensure_session_materialized(&paths, &id)?;
            let now = Utc::now();
            upsert_catalog_entry(&paths, &id, &title, "", now, now)?;
            set_active_session(&paths, &id)?;
            *active.write().map_err(|_| CoreError::Io("lock poisoned".into()))? = id.clone();
            hydrate_transcript(&cmds_for_handler, &[])?;
            Ok(serde_json::json!({ "id": id }))
        }),
    )
}

fn register_load(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "load".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "session_id": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "session_id": "string" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("load: session_id required".into()))?
                .to_owned();
            let catalog: Catalog = if paths.catalog.exists() {
                read_json(&paths.catalog)?
            } else {
                Catalog::default()
            };
            let in_catalog = catalog.sessions.iter().any(|e| e.id == session_id);
            if !in_catalog && !session_has_artifacts(&paths, &session_id) {
                return Err(CoreError::Io(format!("session not found: {session_id}")));
            }
            ensure_session_materialized(&paths, &session_id)?;
            set_active_session(&paths, &session_id)?;
            *active.write().map_err(|_| CoreError::Io("lock poisoned".into()))? = session_id.clone();
            let messages = read_session_messages(&paths, &session_id)?;
            rebuild_rail_from_messages(&paths, &session_id, &messages)?;
            hydrate_transcript(&cmds_for_handler, &messages)?;
            Ok(serde_json::json!({ "ok": true, "session_id": session_id }))
        }),
    )
}

fn register_active(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "active".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "session_id": "string", "entry": "object?" }"#.into(),
            callable_by: vec!["panel".into(), "context-editor".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let sid = active.read().unwrap().clone();
            let catalog: Catalog = if paths.catalog.exists() {
                read_json(&paths.catalog)?
            } else {
                Catalog::default()
            };
            let entry = catalog.sessions.iter().find(|e| e.id == sid).cloned();
            Ok(serde_json::json!({ "session_id": sid, "entry": entry }))
        }),
    )
}

fn register_update(
    paths: StorePaths,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "update".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "session_id": "string", "title": "string?", "summary": "string?" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "session": "object" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("update: session_id required".into()))?
                .to_owned();
            let title = input.get("title").and_then(|v| v.as_str());
            let summary = input.get("summary").and_then(|v| v.as_str());
            if title.is_none() && summary.is_none() {
                return Err(CoreError::Io(
                    "update: at least one of title or summary required".into(),
                ));
            }
            update_catalog_entry(&paths, &session_id, title, summary)?;
            let catalog: Catalog = read_json(&paths.catalog)?;
            let session = catalog
                .sessions
                .iter()
                .find(|e| e.id == session_id)
                .cloned()
                .ok_or_else(|| CoreError::Io(format!("session not found: {session_id}")))?;
            Ok(serde_json::json!({ "ok": true, "session": session }))
        }),
    )
}

fn register_delete(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "delete".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "session_id": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "deleted": "string", "active_session_id": "string" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let session_id = input["session_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("delete: session_id required".into()))?
                .to_owned();

            let mut catalog: Catalog = if paths.catalog.exists() {
                read_json(&paths.catalog)?
            } else {
                Catalog::default()
            };
            let in_catalog = catalog.sessions.iter().any(|e| e.id == session_id);
            let is_active = active.read().unwrap().clone() == session_id;
            if !in_catalog && !session_has_artifacts(&paths, &session_id) && !is_active {
                return Err(CoreError::Io(format!("session not found: {session_id}")));
            }

            if in_catalog {
                catalog.sessions.retain(|e| e.id != session_id);
                write_json(&paths.catalog, &catalog)?;
            }
            delete_session_files(&paths, &session_id)?;

            let was_active = is_active;
            let next_id = if was_active {
                if let Some(next) = catalog.sessions.first() {
                    let id = next.id.clone();
                    ensure_session_materialized(&paths, &id)?;
                    set_active_session(&paths, &id)?;
                    *active.write().map_err(|_| CoreError::Io("lock poisoned".into()))? = id.clone();
                    let messages = read_session_messages(&paths, &id)?;
                    rebuild_rail_from_messages(&paths, &id, &messages)?;
                    hydrate_transcript(&cmds_for_handler, &messages)?;
                    id
                } else {
                    create_empty_session(&paths, &active, &cmds_for_handler, "New chat")?
                }
            } else {
                active.read().map_err(|_| CoreError::Io("lock poisoned".into()))?.clone()
            };

            Ok(serde_json::json!({
                "ok": true,
                "deleted": session_id,
                "active_session_id": next_id,
            }))
        }),
    )
}

fn register_edit_message(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "sessions".into(),
                action: "edit-message".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "message_id": "string", "content": "string" }"#.into(),
            output_schema: r#"{ "fork_id": "string?", "truncated": "number" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let message_id = input["message_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("edit-message: message_id required".into()))?
                .to_owned();
            let content = input["content"]
                .as_str()
                .ok_or_else(|| CoreError::Io("edit-message: content required".into()))?
                .to_owned();

            let transcript = cmds_for_handler.invoke(
                "session-store",
                &CommandId::parse("transcript:get@1").unwrap(),
                serde_json::json!({}),
            )?;
            let messages = transcript["messages"]
                .as_array()
                .ok_or_else(|| CoreError::Io("edit-message: bad transcript".into()))?
                .clone();

            let idx = messages
                .iter()
                .position(|m| m["id"].as_str() == Some(message_id.as_str()))
                .ok_or_else(|| CoreError::Io(format!("message not found: {message_id}")))?;

            if messages[idx]["role"].as_str() != Some("user") {
                return Err(CoreError::Io(
                    "edit-message: only user messages can be edited in v1".into(),
                ));
            }

            let session_id = active.read().unwrap().clone();
            let mut fork_id = None;
            let truncated;

            let mut new_messages = messages.clone();
            if idx + 1 < messages.len() {
                let fork_uuid = Uuid::new_v4().to_string();
                let fork_file_name = format!("fork-{fork_uuid}.jsonl");
                let branch_dir = paths.branch_dir(&session_id);
                std::fs::create_dir_all(&branch_dir).map_err(io_err)?;
                let branch_path = branch_dir.join(&fork_file_name);
                rewrite_session_file_paths(&branch_path, &messages[..=idx])?;
                let branch_rel = format!(
                    "human/branches/{session_id}/{fork_file_name}"
                );
                let preview = format!("Fork before edit: {}", message_preview(&messages[idx]));
                append_fork_marker(
                    &paths,
                    &session_id,
                    &message_id,
                    &fork_uuid,
                    &branch_rel,
                    &preview,
                    messages.len(),
                )?;
                fork_id = Some(fork_uuid);
            }

            if let Some(obj) = new_messages[idx].as_object_mut() {
                obj.insert("content".into(), serde_json::json!(content));
            }
            truncated = new_messages.len().saturating_sub(idx + 1);
            new_messages.truncate(idx + 1);

            rewrite_session_file(&paths, &session_id, &new_messages)?;
            hydrate_transcript(&cmds_for_handler, &new_messages)?;

            Ok(serde_json::json!({
                "fork_id": fork_id,
                "truncated": truncated,
            }))
        }),
    )
}

fn rewrite_session_file_paths(path: &Path, messages: &[serde_json::Value]) -> Result<(), CoreError> {
    let mut f = std::fs::File::create(path).map_err(io_err)?;
    for msg in messages {
        let line = serde_json::to_string(msg).map_err(json_err)?;
        writeln!(f, "{line}").map_err(io_err)?;
    }
    Ok(())
}

fn register_rail_list(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "human-rail".into(),
                action: "list".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "markers": "array", "session_id": "string" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let sid = active.read().unwrap().clone();
            let rail = read_rail(&paths, &sid)?;
            Ok(serde_json::json!({
                "session_id": sid,
                "markers": rail.markers,
            }))
        }),
    )
}

fn register_rail_add_marker(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "human-rail".into(),
                action: "add-marker".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "message_id": "string", "symbol": "string", "note": "string?" }"#
                .into(),
            output_schema: r#"{ "id": "string" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let message_id = input["message_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("add-marker: message_id required".into()))?
                .to_owned();
            let symbol = input["symbol"]
                .as_str()
                .unwrap_or("star")
                .to_owned();
            let note = input["note"].as_str().map(str::to_owned);
            let sid = active.read().unwrap().clone();
            let mut rail = read_rail(&paths, &sid)?;
            let id = Uuid::new_v4().to_string();
            rail.markers.push(RailMarker {
                id: id.clone(),
                kind: "user".into(),
                marker_type: "bookmark".into(),
                message_id,
                fork_id: None,
                symbol: Some(symbol),
                preview: String::new(),
                note,
                branch_file: None,
            });
            write_rail(&paths, &rail)?;
            Ok(serde_json::json!({ "id": id }))
        }),
    )
}

fn register_branch_list(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "human-branch".into(),
                action: "list".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "forks": "array" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let sid = active.read().unwrap().clone();
            let index_path = paths.fork_index_file(&sid);
            let index: ForkIndex = if index_path.exists() {
                read_json(&index_path)?
            } else {
                ForkIndex {
                    session_id: sid,
                    forks: vec![],
                }
            };
            Ok(serde_json::json!({ "forks": index.forks }))
        }),
    )
}

fn register_branch_open(
    paths: StorePaths,
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let _ = cmds;
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "human-branch".into(),
                action: "open".into(),
                version: 1,
            },
            owner: "session-store".into(),
            input_schema: r#"{ "fork_id": "string" }"#.into(),
            output_schema: r#"{ "messages": "array", "fork": "object" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let fork_id = input["fork_id"]
                .as_str()
                .ok_or_else(|| CoreError::Io("open: fork_id required".into()))?
                .to_owned();

            let sid = active.read().unwrap().clone();
            let index_path = paths.fork_index_file(&sid);
            let index: ForkIndex = read_json(&index_path)?;
            let fork = index
                .forks
                .iter()
                .find(|f| f.id == fork_id)
                .ok_or_else(|| CoreError::Io(format!("fork not found: {fork_id}")))?
                .clone();

            let branch_path = paths.root.join(".nulqor").join(&fork.file);
            let messages = read_session_messages_at(&branch_path)?;
            Ok(serde_json::json!({
                "messages": messages,
                "fork": fork,
            }))
        }),
    )
}

fn read_session_messages_at(path: &Path) -> Result<Vec<serde_json::Value>, CoreError> {
    if !path.exists() {
        return Err(CoreError::Io(format!("branch file missing: {}", path.display())));
    }
    let f = std::fs::File::open(path).map_err(io_err)?;
    let reader = BufReader::new(f);
    let mut out = Vec::new();
    for line in reader.lines() {
        let line = line.map_err(io_err)?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        out.push(serde_json::from_str(trimmed).map_err(json_err)?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// IO helpers
// ---------------------------------------------------------------------------

fn resolve_workspace_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if cwd.join("extensions").exists() || cwd.join("AGENTS.md").exists() {
        return cwd;
    }
    if let Some(parent) = cwd.parent() {
        if parent.join("extensions").exists() || parent.join("AGENTS.md").exists() {
            return parent.to_path_buf();
        }
    }
    cwd
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, CoreError> {
    let raw = std::fs::read_to_string(path).map_err(io_err)?;
    serde_json::from_str(&raw).map_err(json_err)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), CoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err)?;
    }
    let raw = serde_json::to_string_pretty(value).map_err(json_err)?;
    std::fs::write(path, raw).map_err(io_err)
}

fn io_err(e: std::io::Error) -> CoreError {
    CoreError::Io(e.to_string())
}

fn json_err(e: serde_json::Error) -> CoreError {
    CoreError::Io(e.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> StorePaths {
        let base = std::env::temp_dir().join(format!("nulqor-session-test-{}", Uuid::new_v4()));
        let paths = StorePaths::new(base);
        paths.ensure_dirs().unwrap();
        paths
    }

    #[test]
    fn update_catalog_entry_changes_title_and_summary() {
        let paths = temp_store();
        let id = "test-session";
        upsert_catalog_entry(
            &paths,
            id,
            "Old title",
            "Old summary",
            Utc::now(),
            Utc::now(),
        )
        .unwrap();
        update_catalog_entry(&paths, id, Some("New title"), Some("New summary")).unwrap();
        let catalog: Catalog = read_json(&paths.catalog).unwrap();
        assert_eq!(catalog.sessions[0].title, "New title");
        assert_eq!(catalog.sessions[0].summary, "New summary");
    }

    #[test]
    fn ensure_session_materialized_creates_missing_jsonl() {
        let paths = temp_store();
        let id = "stub-session";
        upsert_catalog_entry(&paths, id, "Stub", "", Utc::now(), Utc::now()).unwrap();
        assert!(!paths.session_file(id).exists());
        ensure_session_materialized(&paths, id).unwrap();
        assert!(paths.session_file(id).exists());
        assert!(paths.rail_file(id).exists());
    }

    #[test]
    fn delete_session_removes_catalog_and_files() {
        let paths = temp_store();
        let id = "to-delete";
        upsert_catalog_entry(&paths, id, "Delete me", "", Utc::now(), Utc::now()).unwrap();
        rewrite_session_file(&paths, id, &[]).unwrap();
        init_rail(&paths, id).unwrap();
        delete_session_files(&paths, id).unwrap();
        let mut catalog: Catalog = read_json(&paths.catalog).unwrap();
        catalog.sessions.retain(|e| e.id != id);
        write_json(&paths.catalog, &catalog).unwrap();
        let catalog: Catalog = read_json(&paths.catalog).unwrap();
        assert!(catalog.sessions.is_empty());
        assert!(!paths.session_file(id).exists());
        assert!(!paths.rail_file(id).exists());
    }
}
