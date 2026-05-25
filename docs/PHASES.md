# Nulqor — Current Phase Status

Quick-reference for building agents. Full spec in `BUILD_PLAN.md`.

---

## Current: Phase 2 — First AI Harness ✅ GATE PASSED

**Goal:** All AI behavior as extensions — no core changes.

| Task | Status |
|---|---|
| 2.1 `provider-lmstudio` — LM Studio, single-flight queue, stream events (4 tests) | ✅ Done |
| 2.2 `transcript` — shared session, message schema, stream-done subscription (3 tests) | ✅ Done |
| 2.3 `http-api` — axum HTTP+WebSocket, observer/catch-up protocol (6 tests) | ✅ Done |
| 2.4 `chat-panel` — TypeScript streaming UI, reasoning blocks, token budget | ✅ Done |
| 2.5 `context-editor` — skills/agents/rules, system prompt assembly, hot-reload (4 tests) | ✅ Done |
| 2.6 `mcp-bridge` — 5 MCP tools, NULQOR_API_URL, boundary enforcement (2 tests) | ✅ Done |

**Gate verdict:** `cargo test --workspace` → 63/63 pass. HTTP API on port 8080 with full decisions/006 §1–3 surface. Linter clean on all Phase 2 extension manifests. No core files modified.

---

## Up Next: Phase 3 — Prove the loop closes

The thesis test: pick one thing the Subject model reliably fails, author one artifact (skill/rule), show it now passes and holds on a second related task.

**Known candidate (from `harness/runs/2026-05-24.jsonl`):** Gemma 4 E4B fails temporal questions (turns 3, 6, 7, 8, 18). Fix: inject current date into system prompt context.

Tasks:
1. Skill runner — `load_skill` tool, inject full skill body on demand
2. Validation extension — deterministic pass/fail for bounded task checks
3. Loop closure demo (human-guided)
4. Run logging — `runs/YYYY-MM-DD.jsonl`

**Gate:** Documented before/after with run logs proving one captured artifact closed a Subject failure.

---

## Upcoming

- **Phase 4:** Persistence (SQLite), project files, agent loop, context manager
- **Phase 5+:** A/B compare, bake/export, memory, linter UI, model routing

See `BUILD_PLAN.md` for full task lists and gates.
