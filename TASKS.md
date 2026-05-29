# TASKS — Nulqor

Active task queue. Read before starting non-trivial work. Full phase specs in `docs/BUILD_PLAN.md`.

---

## Phase 0 — Skeleton & Guardrails

| # | Task | Status | Notes |
|---|---|---|---|
| 0.1 | Create Tauri 2 + Rust + TypeScript project; window opens titled "Nulqor" | ✅ Done | `src-tauri/`, `src/`, `package.json` |
| 0.2 | Repo layout: `docs/`, `extensions/`, `skills/`, `TASKS.md`, `.gitignore` | ✅ Done | Planning docs in `docs/`; superseded copies in `archive/new-plans/` |
| 0.3 | Write `tools/nulqor-lint` binary | ✅ Done | 12/12 tests pass |
| 0.4 | CI: GitHub Actions builds app on all target OSes, runs linter | ⬜ Pending | |

**Gate 0: ✅ PASSED**

---

## Phase 1 — Frozen Core (eight responsibilities)

| # | Task | Status | Notes |
|---|---|---|---|
| 1.1 | `version.rs` — three-axis version manager | ✅ Done | 5 tests |
| 1.2 | `events.rs` — namespace-scoped event bus | ✅ Done | 5 tests |
| 1.3 | `commands.rs` — versioned command registry | ✅ Done | 5 tests |
| 1.4 | `permission.rs` — four-class permission gate | ✅ Done | 6 tests |
| 1.5 | `capability.rs` — scoped fs/http/sidecar capability layer | ✅ Done | 4 tests |
| 1.6 | `runtime.rs` — owned Tokio runtime | ✅ Done | 3 tests |
| 1.7 | `loader.rs` — discover → lint → dep-sort → activate | ✅ Done | 4 tests |
| 1.8 | `ipc.rs` — Tauri IPC bridge | ✅ Done | |
| 1.9 | `extensions/host/` — `canvas:status@1`, emits `canvas:ready@1` | ✅ Done | |
| 1.10 | `extensions/hello-panel/` — `hello:ping@1`, subscribes `canvas:ready@1` | ✅ Done | |

**Gate 1: ✅ PASSED** — 44/44 tests.

---

## Phase 2 — First AI Harness (extensions only)

| # | Task | Status | Notes |
|---|---|---|---|
| 2.1 | `extensions/provider-lmstudio/` — slotted provider, single-flight queue | ✅ Done | 4 tests; background generate task |
| 2.2 | `extensions/transcript/` — shared session, message schema, JSONL schema | ✅ Done | 3 tests; stream-done subscription |
| 2.3 | `extensions/http-api/` — axum HTTP + WebSocket, observer/catch-up | ✅ Done | 6 tests; full decisions/006 §1–3 surface |
| 2.4 | `extensions/chat-panel/` — TypeScript chat UI, streaming, reasoning blocks | ✅ Done | `ui/main.ts`, `ui/style.css` |
| 2.5 | `extensions/context-editor/` — skills/agents/rules, system prompt, hot-reload | ✅ Done | 4 tests; YAML frontmatter, file watcher |
| 2.6 | `extensions/mcp-bridge/` — stdio MCP proxy, 5 tools | ✅ Done | 2 tests; NULQOR_API_URL override |

**Gate 2: ✅ PASSED** — 63/63 tests across workspace. HTTP API on port 8080. Observer/catch-up protocol per decisions/006 §3. Context editor assembles system prompt in correct order. Linter clean on all Phase 2 manifests (only `sample-broken` fails, as designed). No core files modified.

---

## Phase 3 — Prove the loop closes (the thesis test)

| # | Task | Status | Notes |
|---|---|---|---|
| 3.1 | `extensions/skill-runner/` — `skill-runner:load@1`, `skill-runner:list@1` | ✅ Done | 2 tests; delegates to context-editor, logs each invocation |
| 3.2 | `extensions/validation/` — `validation:check@1` with 7 check types | ✅ Done | 9 tests; contains/exact/not_empty/regex/json/is_date_like |
| 3.3 | Loop closure demonstration (human + Builder + Subject model) | ⬜ Pending | Infrastructure ready; requires human to run Subject model session |
| 3.4 | `extensions/run-logger/` — JSONL append per turn to `runs/YYYY-MM-DD.jsonl` | ✅ Done | 2 tests; subscribes to `transcript:message-added@1` |
| 3.3 prep | `rules/current-date.md` + `{{current_datetime}}` interpolation in context-editor | ✅ Done | Temporal fix artifact; dynamic date injected into every system prompt |

**Gate 3:** Documented before/after where one captured artifact turned a Subject failure into a repeatable success. 3.3 is the human-driven step.

---

## Phase 4 — Persistence & harness essentials

**Sessions v1 (partial):** `session-store` + `chat-panel` file persistence shipped ahead of Phase 4. Read [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md) before continuing; SQLite/FTS remains indexer-only (not a second truth store).

| # | Task | Status | Notes |
|---|---|---|---|
| 4.1 | Persistence extension — SQLite + FTS5 | ⬜ Pending | Index over `.nulqor/` files; see decision 009 |
| 4.2 | Project save/load — `.nulqor` files | 🟡 Partial | v1: `sessions/*.jsonl` + `human/**`; room mode, search, CLI parity in BACKLOG |
| 4.3 | Agent-loop extension — plan→act→observe→verify | ⬜ Pending | |
| 4.4 | Context manager — token budget + compaction | ⬜ Pending | |
| 4.5 | Decision records workflow — `docs/decisions/<NNN>.md` command | ⬜ Pending | |

---

## Decisions needed

- None currently.

---

## Recently completed

- 2026-05-24: **Sessions v1** — `session-store` (file persistence, human rail, fork-on-edit) + `chat-panel` (rail, session picker, fork overlay). Handoff: [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md).
- 2026-05-24: MCP stdio server (`tools/mcp-server/`) + `.cursor/mcp.json` — Builder agent can now join conversations.
- 2026-05-24: Phase 3 (3.1, 3.2, 3.4 + temporal artifact) — skill runner, validation, run logger, date rule. 11 new tests. 3.3 awaits human demo run.
- 2026-05-24: Phase 2 (2.1–2.6) — full AI harness as extensions. 63 tests pass.
- 2026-05-24: Phase 1 — Frozen Core (1.1–1.10) + linter lib refactor. 44 tests pass.
- 2026-05-24: Phase 0 (0.1, 0.2, 0.3) bootstrapped from Go/Wails proof-of-concept.
