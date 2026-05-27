# Extensions Index

This directory contains all Nulqor extensions. Each extension is a sealed capability object
with a mandatory `extension.toml` manifest, validated by `tools/nulqor-lint` before load.

## Registered Extensions

Each extension is self-contained under `extensions/<id>/`. Rust sources compile into the core via `src-tauri/src/extensions/mod.rs` (`#[path]` includes).

| Id | Kind | Status | Purpose |
|---|---|---|---|
| `clock-panel` | Panel | Phase 1 | Live clock tile for multi-panel canvas testing |
| `host` | Host | Phase 1 | Transparent canvas shell — grid desk, menu bar, panel tiles |
| `hello-panel` | Panel | Phase 1 | Sample panel — proves extension contract end-to-end |
| `hello-world` | Panel | Phase 1 | Minimal "Hello World" window — startup profile demo |
| `provider-lmstudio` | Service | Phase 2 | LM Studio connection, model list, single-flight generation |
| `transcript` | Service | Phase 2 | Shared in-memory session; emits `transcript:message-added@1` |
| `session-store` | Service | Phase 4 prep | File sessions (`.nulqor/sessions/`), human rail + archived forks |
| `http-api` | Service | Phase 2 | HTTP/WebSocket API + observer/catch-up protocol |
| `chat-panel` | Panel | Phase 2 | Dominant chat UI with streaming, reasoning, token stats |
| `context-editor` | Service | Phase 2 | Loads skills/agents/rules, assembles system prompt, hot-reloads |
| `mcp-bridge` | Service | Phase 2 | stdio MCP proxy to the HTTP API |
| `skill-runner` | Service | Phase 3 | On-demand skill loading and injection with execution logging |
| `validation` | Service | Phase 3 | Deterministic pass/fail checks on model output |
| `run-logger` | Service | Phase 3 | Appends every turn to `runs/YYYY-MM-DD.jsonl` |

## Extension Scaffold (required structure for every extension)

**Create new extensions only via** `skills/create-extension/scripts/create.ps1`.  
**Verify after any layout change:** `skills/audit-project/scripts/audit.ps1`

```
extensions/<id>/
  extension.toml   â† manifest (required; linter enforces schema)
  README.md        â† purpose, commands, events, failure modes
  src/lib.rs       â† Rust implementation (compiled via src-tauri bridge)
  ui/              â† TypeScript panel (Panel kind only; at least one file)
  tests/           â† behaviour tests
  fixtures/        â† sample inputs + expected outputs
```

### Layout contract (audit-enforced)

- Rust implementation **only** in `src/lib.rs` â€” never `src-tauri/src/ext_*.rs`
- Panel UI **only** in `ui/` â€” never repo root `src/*.ts`
- Folder name = `extension.toml` `id` = row in this index = `#[path]` in `mod.rs` = `loader.register` in `lib.rs`

See `docs/DESIGN.md Â§5` for the manifest schema and `docs/BUILD_PLAN.md Â§21` for scaffold rules.
