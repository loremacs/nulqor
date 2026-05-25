# PROJECT_FEATURES.md — Nulqor

Canonical record of every shipped feature. Each entry provides enough implementation detail to reconstruct the feature from scratch.

---

## Phase 0 — Skeleton & Guardrails

### 0.1 Tauri 2 + Rust + TypeScript project
**Files:** `src-tauri/`, `src/`, `package.json`, `src-tauri/tauri.conf.json`
**Purpose:** Minimal Tauri app that opens a window titled "Nulqor". No product behaviour.
**Key config:** `bundle.active = false`, `bundle.icon = []` (avoids Windows icon requirement in dev). `npm start` runs `tauri dev`.

### 0.2 Repo layout
**Files:** `docs/`, `extensions/`, `skills/`, `rules/`, `tools/`, `archive/`, `TASKS.md`, `AGENTS.md`, `README.md` (slim build entry)
**Index files:** Each main content area has `index.md`. `AGENTS.md` links to all of them.
**Extension impl:** `extensions/<id>/src/lib.rs`, compiled via `src-tauri/src/extensions/mod.rs` (`#[path]` bridge).
**Purpose:** Enforced structure from `DESIGN.md §10`. Max depth 5 from repo root (exception: `skills/<n>/scripts/<f>` at depth 6).
**Archive:** Superseded monolithic README at `archive/product-brief-monolith.md`.
**Runtime logs:** `runs/` for `run-logger`; `runs/*.jsonl` gitignored.

### 0.3 Linter (`tools/nulqor-lint`)
**Files:** `tools/nulqor-lint/src/main.rs`, `tools/nulqor-lint/src/lib.rs`
**Purpose:** Mechanically enforces manifest schema, naming rules (`namespace:action@version`), directory depth, and boundary rules. Prints `FAIL: <file>: <reason>` — never prose.
**Key functions:** `lint_extension(dir) → Vec<LintError>`. Run: `cargo run --manifest-path tools/nulqor-lint/Cargo.toml -- <dir>`.

### 0.4 Project layout audit (`skills/audit-project`)
**Files:** `skills/audit-project/scripts/audit.ps1`, `skills/audit-project/skill.md`
**Purpose:** Enforces colocated extension layout, forbidden legacy paths, registry sync (disk ↔ `extensions/index.md` ↔ `mod.rs` ↔ `lib.rs`), and invokes `nulqor-lint`.
**Run:** `skills/audit-project/scripts/audit.ps1 -Quiet`

### 0.5 Extension scaffold skill (`skills/create-extension`)
**Files:** `skills/create-extension/scripts/create.ps1`, `skills/create-extension/skill.md`
**Purpose:** Creates `extensions/<id>/` with `extension.toml`, `README.md`, `src/lib.rs`, optional `ui/`; appends `mod.rs` bridge and index row. Prints required `loader.register` line for `lib.rs`.
**Run:** `skills/create-extension/scripts/create.ps1 -Id <id> -Kind Service -Purpose "..."`

### 0.6 Communicate with running app (`skills/nulqor-communicate`)
**Files:** `skills/nulqor-communicate/scripts/chat.ps1`, `skills/nulqor-communicate/skill.md`
**Purpose:** Documents all external communication paths (HTTP :8080, MCP stdio, mcp-bridge commands, WebSocket, Tauri IPC). Script wraps observer register + send + transcript poll.
**Run:** `skills/nulqor-communicate/scripts/chat.ps1 -Action send -Message "..."`

### Runtime: `block_on_compat` (`runtime.rs`)
**Purpose:** `provider:connect@1` and `provider:models@1` (and mcp-bridge HTTP helpers) must work when invoked from the async HTTP API. `block_on_compat` uses `tokio::task::block_in_place` + `handle().block_on` when already on a Tokio worker.
**Key function:** `Runtime::block_on_compat(fut)` — used by `provider-lmstudio` and `mcp-bridge`.

### 0.7 Startup profile (`nulqor.toml`) + canvas shell (`host`)
**Files:** `nulqor.toml`, `src-tauri/src/startup_config.rs`, `extensions/host/ui/`
**Purpose:** Root config selects enabled extensions and initial open panels; host shell is always the window UI.
**Keys:** `open_panels`, `enabled_extensions`, `[shell]` (`grid_cols`, `grid_rows`, `snap_enabled`, `show_grid`).
**Shell:** Transparent fullscreen window; menu bar + panel tiles are interactive. OS desktop click-through deferred until Tauri supports event forwarding with `setIgnoreCursorEvents`.
**Commands:** `canvas:config@1` returns startup shell config + discovered Panel extensions.
**Panel contract:** `extensions/<id>/ui/panel.ts` exports `mount(container)`; registered in `host/ui/panels.ts`.

### 0.8 Hello World panel (`hello-world`)
**Files:** `extensions/hello-world/extension.toml`, `src/lib.rs`, `ui/main.ts`, `ui/style.css`
**Purpose:** Minimal Panel — centers "Hello World" in the main window. Requires `host`.
**Activate:** Set `entry_panel = "hello-world"` and include in `enabled_extensions` in `nulqor.toml`.

---

### 1.1 Version manager (`version.rs`)
**Purpose:** Three-axis versioning (core API, manifest schema, individual contract). Per-contract coexistence of `@1` and `@2`. Fails loud on unknown version.
**Key functions:** `VersionManager::check_extension(manifest)`, `available_versions(base_key)`.

### 1.2 Event bus (`events.rs`)
**Purpose:** Namespace-scoped publish/subscribe. Non-matching subscribers never woken.
**Key functions:** `EventBus::publish(event)`, `subscribe(pattern, handler) → SubscriptionId`, `unsubscribe(id)`.
**Pattern matching:** `EventPattern::exact(ns, name, version)` or wildcard.

### 1.3 Command registry (`commands.rs`)
**Purpose:** Register and invoke commands by `namespace:action@version`. Ownership + permission enforced. Duplicate registration fails loud. Version mismatch fails loud with list of available versions.
**Key functions:** `CommandRegistry::register(decl, handler)`, `invoke(caller, id, input)`, `list_commands()`.

### 1.4 Permission gate (`permission.rs`)
**Purpose:** Four classes — `Read` (always allowed), `Write` (allowed, logged), `Destructive` (requires confirm hook), `System` (always denied in Phase 1).
**Key functions:** `PermissionGate::check(caller, permission, command_id)`, `set_confirm_hook(hook)`.

### 1.5 Capability layer (`capability.rs`)
**Purpose:** Scoped `fs_read`/`fs_write` (path-restricted), declared-host `http_request`, `spawn_sidecar` (behind `system`). Extensions must declare fs_scopes and http_hosts in manifest.
**Key functions:** `Capabilities::fs_read(ext_id, path)`, `check_http_allowed(ext_id, url)`.

### 1.6 Async runtime owner (`runtime.rs`)
**Purpose:** One explicit multi-threaded Tokio runtime. Extensions do not create their own runtimes.
**Key functions:** `Runtime::new()`, `spawn_task(budget, fut)`, `spawn_compute(job)`, `block_on(fut)`.
**Critical:** `block_on` panics if called from within an async Tokio context. Use only from `activate()` and sync command handlers.

### 1.7 Extension loader (`loader.rs`)
**Purpose:** Discover `extension.toml` manifests, run linter, topological-sort by `requires`, call `activate()`. Factory pattern: `loader.register(id, fn)`.
**Key functions:** `Loader::scan_and_load(extensions_dir, root, ctx, enabled_filter)`, `expand_enabled_with_deps`.

### 1.8 IPC bridge (`ipc.rs`)
**Purpose:** Routes Tauri `invoke` calls to command registry. Exposes scoped event bus to frontend.
**Tauri commands:** `core_invoke(namespace, action, version, input)`, `core_list_commands()`.

### 1.9 Host extension (`extensions/host/`)
**Purpose:** Transparent canvas shell — grid desk, draggable menu bar (Settings/Apps), panel tiles. Emits `canvas:ready@1` via core after load.
**Commands:** `canvas:status@1`, `canvas:config@1`.
**UI:** `extensions/host/ui/` — always loaded from root `index.html`.

### 1.10 Hello-panel extension (`extensions/hello-panel/`)
**Purpose:** Trivial panel that registers `hello:ping@1` and subscribes to `canvas:ready@1`. Proves extension contract from Rust and TypeScript sides.

---

## Phase 2 — First AI Harness

### 2.1 Provider extension (`extensions/provider-lmstudio/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** Slotted `provider` capability, instance `lmstudio`. Connects to LM Studio, fetches model list from `GET /v1/models`, streams via Chat Completions API. Single-flight request queue (only one generation at a time).
**Commands:** `provider:connect@1({ url })`, `provider:models@1()`, `provider:generate@1({ messages, system_prompt, stream_id?, model? })`.
**Events emitted:** `provider:stream-start@1`, `provider:stream-delta@1 { delta }`, `provider:stream-done@1 { content, reasoning, tokens, model }`, `provider:stream-error@1`.
**Key constraints:** URL must be localhost. Model id fetched from `/v1/models` — never hardcoded.

### 2.2 Transcript extension (`extensions/transcript/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** One shared in-memory session. Message schema per decisions/006 §5: `id`, `role`, `content`, `timestamp`, `model`, `latency_ms`, `tokens`, `driver`, `participant_name`, `reasoning?`, `agent?`.
**Commands:** `transcript:get@1()`, `transcript:add-user-message@1({ content, participant_name, agent?, driver? })`, `transcript:clear@1()`, `transcript:set-active-agent@1({ agent })`.
**Events emitted:** `transcript:message-added@1 { message }`, `transcript:cleared@1`.
**Subscribes:** `provider:stream-done@1` → appends assistant turn, emits `transcript:message-added@1`.

### 2.3 HTTP API extension (`extensions/http-api/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** axum HTTP server (port 8080) + WebSocket. Exact endpoint surface from decisions/006 §1–3.
**Endpoints:** `GET /health`, `POST /connect`, `GET /models`, `POST /message`, `POST /observers/register`, `GET /observers/catch-up`, `POST /observers/ack`, `GET /observers`, `GET /transcript`, `GET /ws/transcript`, `GET /ws/chat`.
**Observer protocol:** First `catch_up` returns full backlog. Subsequent calls return only new turns. Duplicate observer name is idempotent. Unregistered observer on `/message` → 400.

### 2.4 Chat UI panel (`extensions/chat-panel/`)
**Files:** `extension.toml`, `src/lib.rs`, `ui/main.ts`, `ui/style.css`. Root `index.html` loads the panel UI.
**Purpose:** Dominant transcript view + input box + connection bar. Streams reply tokens live. Collapsible system prompt and reasoning blocks. Fixed harness token cost per turn. Participant labels per decisions/006 §4.
**Key behaviour:** Calls `context-editor:system-prompt@1` before each generation. Model dropdown populated from `/v1/models`. Connects via `provider:connect@1`.

### 2.5 Context editor extension (`extensions/context-editor/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** Loads skills (`skills/<name>/SKILL.md` YAML frontmatter), agents (`AGENTS.md`, `agents/<n>.md`), rules (`rules/*.{md,mdc,txt}`, alphabetical). Assembles system prompt in order: persona → rules → skill index. Hot-reloads on file change via `notify` watcher. Interpolates `{{current_date}}` and `{{current_datetime}}` in all text at assembly time.
**Commands:** `context-editor:reload@1()`, `context-editor:list-skills@1()`, `context-editor:list-agents@1()`, `context-editor:list-rules@1()`, `context-editor:load-skill@1({ name })`, `context-editor:system-prompt@1({ agent? })`.
**CWD note:** In dev mode cargo sets CWD to `src-tauri/`. `resolve_workspace_root()` walks up to the workspace root automatically.

### 2.6 MCP bridge extension (`extensions/mcp-bridge/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** stdio MCP proxy to the HTTP API. Does not embed the engine — app must be running.
**Tools (commands):** `mcp-bridge:register_observer@1`, `mcp-bridge:catch_up@1`, `mcp-bridge:ack_observer@1`, `mcp-bridge:send_message@1`, `mcp-bridge:list_observers@1`.
**Config:** `NULQOR_API_URL` env var overrides default `http://localhost:8080`.

---

## Phase 3 — Compounding Loop Infrastructure

### 3.1 Skill-runner extension (`extensions/skill-runner/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** On-demand skill loading with execution logging. Wraps `context-editor:load-skill@1` so every skill invocation is traced to stderr. Missing skill returns `{ found: false }` rather than an error.
**Commands:** `skill-runner:load@1({ name }) → { name, body, found }`, `skill-runner:list@1() → { skills }`.
**Dependency:** Requires `context-editor` to be loaded first.

### 3.2 Validation extension (`extensions/validation/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** Deterministic pass/fail check on model output. Returns short structured result the model can read and act on.
**Command:** `validation:check@1({ type, actual, expected? }) → { pass, reason }`.
**Check types:** `contains`, `not_contains`, `exact`, `not_empty`, `matches_regex` (string contains), `is_valid_json`, `is_date_like` (year 2000–2099 present).
**No external deps:** Pure string logic, no regex crate needed for Phase 3.

### 3.3 Temporal date artifact (`rules/current-date.md`)
**Purpose:** Fix for the known Subject failure (Gemma 4 E4B turns 3, 6, 7, 8, 18 in harness run). Injects `Current date and time: {{current_datetime}}` into every system prompt.
**Mechanism:** `context-editor` resolves `{{current_datetime}}` (and `{{current_date}}`) at prompt-assembly time via `interpolate_date()`. No code change needed per session — the rule is the entire artifact.
**Validation:** Use `validation:check@1({ type: "is_date_like", actual: <model reply> })` to verify.

### 3.4 Run-logger extension (`extensions/run-logger/`)
**Files:** `extension.toml`, `src/lib.rs`
**Purpose:** Append every turn to `runs/YYYY-MM-DD.jsonl` for before/after comparison. Creates `runs/` dir if absent.
**Subscribes:** `transcript:message-added@1` — one JSONL line per message.
**JSONL fields:** all `Message` fields (id, role, content, timestamp, model, latency_ms, tokens, driver, participant_name, reasoning?).
**File path:** `<workspace_root>/runs/YYYY-MM-DD.jsonl`. Uses `resolve_workspace_root()` same as context-editor.

---

## MCP stdio server (`tools/mcp-server/`)

**Purpose:** Standalone Rust binary that speaks MCP JSON-RPC 2.0 over stdio and proxies the five observer tools to the running Nulqor HTTP API. Allows Cursor (or any MCP-capable IDE agent) to join the shared transcript as an observer.
**Files:** `tools/mcp-server/Cargo.toml`, `tools/mcp-server/src/main.rs`, `.cursor/mcp.json`
**Tools exposed:** `register_observer`, `catch_up`, `ack_observer`, `send_message`, `list_observers`
**Config:** `NULQOR_API_URL` env var (default: `http://localhost:8080`)
**Cursor config:** `.cursor/mcp.json` registers the server as `"nulqor"` — Cursor picks it up automatically on workspace open.
**Protocol version:** `2024-11-05`
**Build:** member of root workspace (`Cargo.toml`). Built with `cargo build --manifest-path tools/mcp-server/Cargo.toml`.
**Usage flow:**
1. Start Nulqor app (`npm start`) — HTTP API comes up on port 8080
2. Cursor auto-launches the MCP server via `.cursor/mcp.json`
3. Agent calls `register_observer` → `catch_up` → `send_message` → `catch_up` loop
**Smoke-test:** `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...}}' | cargo run --manifest-path tools/mcp-server/Cargo.toml --quiet`

---

## Cross-phase patterns

### Extension activation order
Loader topologically sorts by `requires`. Activation order for Phase 3: `host` → `hello-panel` → `provider-lmstudio` → `transcript` → `http-api` → `chat-panel` → `context-editor` → `mcp-bridge` → `skill-runner` → `validation` → `run-logger`.

### Synchronous locking strategy
All shared state uses `std::sync::RwLock` (not `tokio::sync`). This allows command handlers and event subscribers (which are sync closures) to access state without `block_in_place` or async context.

### Async from sync bridging
When a sync handler needs async I/O (e.g., HTTP fetch in `provider-lmstudio`, `mcp-bridge`), it calls `runtime.block_on(async { ... })`. This is safe only from non-async contexts.

### Workspace root resolution
`resolve_workspace_root()` (in `context-editor`, `run-logger`): if CWD is `src-tauri/` (dev mode), returns parent. Checks for `extensions/` or `AGENTS.md` as sentinel.
