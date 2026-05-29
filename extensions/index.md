# Extensions Index

This directory contains all Nulqor extensions. Each extension is a sealed capability object
with a mandatory `extension.toml` manifest, validated by `tools/nulqor-lint` before load.

## Registered Extensions

Each extension is self-contained under `extensions/<id>/`. Rust sources compile into the core via `src-tauri/src/extensions/mod.rs` (`#[path]` includes).

| Id | Kind | Status | Purpose |
|---|---|---|---|
| `workbench` | Panel | Phase 2 | Inspect extensions, commands, skills, rules, and agents with structured editors. |
| `registry` | Service | Phase 2 | Extension graph, manifest introspection, and command catalog for the workbench panel. |
| `clock-panel` | Panel | Phase 1 | Live clock tile for multi-panel canvas testing |
| `host` | Host | Phase 1 | Transparent canvas shell — grid desk, menu bar, panel tiles |
| `hello-panel` | Panel | Phase 1 | Sample panel — proves extension contract end-to-end |
| `hello-world` | Panel | Phase 1 | Minimal "Hello World" window — startup profile demo |
| `provider-lmstudio` | Provider | Phase 2 | LM Studio backend (`lmstudio:*@1`) — load/unload, streaming |
| `provider-ollama` | Provider | Phase 2 | Ollama backend (`ollama:*@1`) — localhost:11434 |
| `provider-llamacpp` | Provider | Phase 2 | llama.cpp server backend (`llamacpp:*@1`) — localhost:8080 |
| `provider-router` | Service | Phase 2 | Routes public `provider:*@1` to active backend (`active_provider` in `nulqor.toml`) |
| `transcript` | Service | Phase 2 | Shared in-memory session; emits `transcript:message-added@1` |
| `session-store` | Service | Phase 4 prep (v1 shipped) | File sessions (`.nulqor/`), human rail + archived forks — **spec:** [`docs/decisions/009-sessions-file-store.draft.md`](../docs/decisions/009-sessions-file-store.draft.md) |
| `http-api` | Service | Phase 2 | HTTP/WebSocket API + observer/catch-up protocol |
| `chat-panel` | Panel | Phase 2 (v1 sessions UI) | Active-branch chat + human rail + fork overlay — **spec:** [`docs/decisions/009-sessions-file-store.draft.md`](../docs/decisions/009-sessions-file-store.draft.md) §11 |
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
