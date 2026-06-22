# Persistence

Provides the slotted capability `storage`/`main`. Maintains a SQLite + FTS5 index over
`.nulqor/sessions/*.jsonl`. The JSONL files remain the truth store; SQLite is an indexer
only and can be rebuilt at any time.

## Commands

| Command | Purpose |
|---|---|
| `storage:search@1` | FTS5 full-text search across all indexed messages |
| `storage:index-session@1` | Index a single session file by id |
| `storage:reindex@1` | Reindex all session JSONL files under `.nulqor/sessions/` |
| `storage:status@1` | Database stats and health |

## Schema

`messages(id, session_id, role, content, timestamp, participant_name, driver, model)` plus
`messages_fts` (FTS5 over `content` + `participant_name`, external-content table).

## Events

Subscribes to `transcript:message-added@1` to index new messages incrementally.

Requires: `transcript`, `session-store`. Filesystem scope: `.nulqor/`.

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest + command declarations |
| `src/lib.rs` | Rust service implementation |
