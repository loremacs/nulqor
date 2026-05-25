# Extensions Index

This directory contains all Nulqor extensions. Each extension is a sealed capability object
with a mandatory `extension.toml` manifest, validated by `tools/nulqor-lint` before load.

## Registered Extensions

| Id | Kind | Status | Purpose |
|---|---|---|---|
| `host` | Service | Phase 1 | Mounts window shell, emits `canvas:ready@1` |
| `hello-panel` | Panel | Phase 1 | Sample panel — proves extension contract end-to-end |
| `provider-lmstudio` | Service | Phase 2 | LM Studio connection, model list, single-flight generation |
| `transcript` | Service | Phase 2 | Shared in-memory session; emits `transcript:message-added@1` |
| `http-api` | Service | Phase 2 | HTTP/WebSocket API + observer/catch-up protocol |
| `chat-panel` | Panel | Phase 2 | Dominant chat UI with streaming, reasoning, token stats |
| `context-editor` | Service | Phase 2 | Loads skills/agents/rules, assembles system prompt, hot-reloads |
| `mcp-bridge` | Service | Phase 2 | stdio MCP proxy to the HTTP API |
| `skill-runner` | Service | Phase 3 | On-demand skill loading and injection with execution logging |
| `validation` | Service | Phase 3 | Deterministic pass/fail checks on model output |
| `run-logger` | Service | Phase 3 | Appends every turn to `runs/YYYY-MM-DD.jsonl` |

## Extension Scaffold (required structure for every extension)

```
extensions/<id>/
  extension.toml   ← manifest (required; linter enforces schema)
  README.md        ← purpose, commands, events, failure modes
  src/             ← Rust core-side implementation
  ui/              ← TypeScript panel (Panel kind only)
  tests/           ← behaviour tests
  fixtures/        ← sample inputs + expected outputs
```

See `docs/DESIGN.md §5` for the manifest schema and `docs/BUILD_PLAN.md §21` for scaffold rules.
