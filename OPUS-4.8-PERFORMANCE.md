# Opus 4.8 — Performance, Flow, Ports & Loops

Companion to [`OPUS-4.8-REVIEW.md`](OPUS-4.8-REVIEW.md). Findings are grounded in source reads of
the extensions and core. Severity is my judgement of impact at the 8 GB-baseline target the project
sets for itself in `docs/DESIGN.md §15`.

---

## 1. Ports

| Service | Default | Source |
|---|---|---|
| HTTP/WebSocket API | `8080` | `extensions/http-api/src/lib.rs:125` |
| llama.cpp backend | `8080` | `extensions/provider-router/src/lib.rs:39`, `extensions/provider-llamacpp/src/lib.rs:34` |
| LM Studio backend | `1234` | `extensions/provider-lmstudio` |
| Ollama backend | `11434` | `extensions/provider-ollama/src/lib.rs:35` |

**HIGH — collision.** Nulqor's own HTTP API and the llama.cpp model server both default to `8080`.
The llama.cpp README even instructs `llama-server --port 8080`
(`extensions/provider-llamacpp/README.md:10`). A user on that backend will either fail to bind the
API or unknowingly point the API at the model server. Move the API default to a non-conflicting port
(8787 is unused here) and make it configurable via `nulqor.toml`.

**LOW — port is hard-coded as a const.** `DEFAULT_PORT` is a `const u16` in `http-api`; the comment
says "(configurable)" but there is no config path shown. Wire it to `[shell]`/a `[http_api]` block in
`nulqor.toml` so the collision fix is user-overridable without a rebuild.

---

## 2. Polling flows (the main resource cost)

### 2a. Click-through cursor polling — HIGH at 60 Hz
`extensions/host/ui/click-through.ts`

- `POLL_MS = 16` (line 6) → `setInterval` ~60×/sec (lines 101-106).
- Each tick runs `updateFromPoll()` which `await`s **multiple** Tauri IPC round-trips:
  `cursorClientCss()`, `win.innerSize()`, `win.scaleFactor()` (lines 153-178).
- This only runs in fullscreen overlay mode with click-through enabled, but while it runs it is a
  continuous ~60 Hz burst of IPC plus `setIgnoreCursorEvents` calls. On the 8 GB-baseline target
  (where the GPU and CPU budget belong to the model), a 60 Hz IPC loop competing with generation is
  the most expensive idle-time cost in the app.

**Fix options:** (a) the `mousemove` handler already exists (line 180) — rely on it and drop the
poll, or poll far less often (e.g. 100 ms) purely as a safety net; (b) cache `innerSize`/`scaleFactor`
and refresh them on resize events rather than every tick; (c) stop the poll whenever a generation is
streaming.

### 2b. Chat panel transcript polling — MEDIUM, and avoidable
`extensions/chat-panel/ui/panel.ts`

- `setInterval(() => void refreshAll(), 2000)` (line 2155). `refreshAll` runs three IPC calls in
  parallel — `loadTranscript` + `loadRail` + `loadSessions` (lines 447-451) — **every 2 s,
  unconditionally**, even when the window is idle and nothing changed.
- While awaiting a reply it polls every 400 ms and forces a **full** transcript re-fetch each time by
  resetting `transcriptHash = ""` (lines 1538-1552), discarding the hash-diff optimization exactly
  when traffic is highest.

The project already ships `/ws/transcript` and `/ws/chat` WebSockets in `http-api`
(`extensions/http-api/src/lib.rs:476-520`), and `BACKLOG.md` explicitly notes "Chat streaming — live
token stream via Tauri events (today: 2s poll)." The infrastructure to remove this polling exists; the
panel just doesn't use it.

**Fix:** drive the panel from transcript events (Tauri event bridge or the WebSocket). Failing that,
stop the 2 s poll when the document is hidden/blurred and keep the hash diff during streaming.

### 2c. Context-editor filesystem watcher — LOW
`extensions/context-editor/src/lib.rs:1111` — `notify` is configured with
`with_poll_interval(Duration::from_secs(2))`, i.e. **poll-based** watching over `skills/`, `rules/`,
`agents/`, `AGENTS.md`, `.nulqor/`. 2 s polling of several trees is cheap but is a fixed background
cost; prefer native (non-poll) notify where the platform supports it and reserve polling as fallback.

### 2d. Clock panel — NONE
`extensions/clock-panel/ui/panel.ts:44` ticks at 1 s. Correct and harmless.

---

## 3. Concurrency & locking

- **Single-flight generation is correct.** Each provider serializes generation behind a
  `tokio::sync::Mutex` (`generation_lock`, e.g. `extensions/provider-ollama/src/lib.rs:26,38`),
  matching `DESIGN.md §8`. Good.
- **Sync `std::sync::RwLock` everywhere for shared state** (registry, sessions, observers, provider
  caches). This is a deliberate, documented choice (`PROJECT_FEATURES.md` "Synchronous locking
  strategy") so sync command handlers and event subscribers avoid async-lock juggling. It is fine
  **provided no handler holds a write lock across blocking network I/O.** Worth an explicit audit:
  confirm provider handlers drop the state lock *before* `block_on` HTTP calls; a write lock held
  across a 120 s generation timeout would stall every other caller of that provider and any code that
  reads its state. (I did not find a clear violation, but the pattern is easy to introduce and there
  is no test guarding it — see `OPUS-4.8-TESTS.md`.)
- **Resource duplication — LOW.** All three provider extensions construct their own reqwest client
  pairs (`http_probe` + `http_generate`) at load even when only one is the active backend. Minor
  memory/socket overhead; consider lazy construction on first connect.

---

## 4. Loops & lifecycle

- **No agent loop yet,** so the `DESIGN.md §13` loop-iteration cap (5–50, default 20) is currently
  unenforced — there is nothing to bound. Acceptable now; it must ship together with the Phase 4
  agent-loop extension, with a test that proves the cap fires. Do not let the loop land first and the
  guard land later.
- **MCP stdio server** reads stdin line-by-line in a blocking loop (`tools/mcp-server/src/main.rs:309`).
  Correct for a stdio JSON-RPC server; no concern.
- **Streaming wait loop** in `chat-panel` (`while Date.now() < deadline` at line 1538) has a deadline
  guard, so it cannot spin forever — good — but see 2b for the full-refetch cost inside it.

---

## 5. Quick wins (highest value / lowest effort)

1. Move HTTP API default off `8080` and make it config-driven. (bug)
2. Stop chat-panel polling when the panel is hidden; keep the hash diff during streaming. (cheap)
3. Raise click-through `POLL_MS` and cache `innerSize`/`scaleFactor`, or rely on `mousemove`. (cheap)
4. Add the lock-held-across-IO audit/test in `OPUS-4.8-TESTS.md`. (safety)
5. Migrate chat-panel to the existing WebSocket/event stream. (larger, removes polling entirely)
