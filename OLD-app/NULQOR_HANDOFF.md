# Nulqor — Agent Handoff Document

> **Purpose:** Share this file with another coding agent to continue work on the project without re-discovering architecture, features, or gotchas.
>
> **App name:** Nulqor (formerly "Local LLM Harness" in early docs). Window title and UI use **Nulqor**. Internal Go module/binary may still be named `harness`.

---

## 1. What this project is

**Nulqor** is a desktop workbench for iterating on **skills**, **agent personas**, and **rules** against a **local LLM** (LM Studio, Gemma 4 recommended). It is the thing being tuned — not the IDE's cloud model.

### Three actors

| Actor | Role |
|-------|------|
| **Model under test** | Gemma (or any model) via LM Studio OpenAI-compatible API at `http://localhost:1234/v1` |
| **Nulqor (harness)** | Holds context, assembles system prompts, runs chat + tool loop, shows results |
| **Driver** | Human (GUI) or external IDE agent (HTTP/MCP) — both mutate the **same live session** |

### Core loop

Edit skills/agents/rules → hot reload → send test prompts → compare runs → keep what works.

---

## 2. Repository structure

```
skillTest/                          # Repo root (workspace)
├── NULQOR_HANDOFF.md               # This file
├── PLAN.md                         # Original full spec (still authoritative for vision)
├── PROJECT_FEATURES.md             # Implementation log (update when shipping features)
├── AGENTS.md                       # Default agent persona (loaded as agent "default")
├── agents/                         # Agent persona files (INDEX.md ignored)
├── skills/                         # Skill folders with SKILL.md + YAML frontmatter
│   ├── build-harness/
│   └── harness-live-transcript/    # Skill for IDE agents syncing via observer API
├── rules/                          # Always-on rules concatenated into system prompt
│   └── external-agent-sync.md      # Required workflow for external IDE agents
└── harness/                        # Go + Wails application
    ├── main.go                     # Wails GUI entry (Title: "Nulqor")
    ├── app.go                      # Wails bindings → engine
    ├── transcript_bridge.go        # Engine events → Wails JSON payloads
    ├── harness.toml                # Config (paths point to repo root content)
    ├── wails.json                  # Wails project config (binary name still "harness")
    ├── cmd/harness/main.go         # CLI: -mode serve | mcp
    ├── frontend/src/               # Plain HTML/CSS/JS UI
    │   ├── index.html
    │   ├── main.js
    │   └── main.css
    ├── internal/
    │   ├── engine/                 # Core: config, loaders, sessions, chat, observers, tools
    │   ├── lmstudio/               # OpenAI-compatible client + streaming
    │   ├── api/                    # HTTP + WebSocket server
    │   └── mcp/                    # MCP stdio proxy → HTTP API
    ├── runs/                       # JSONL run logs (gitignored)
    └── README.md                   # Quick start for developers
```

**Path resolution:** `harness.toml` paths (`../skills`, `../agents`, `../rules`) resolve relative to the config file directory (`harness/`), so content lives at **repo root**, not inside `harness/`.

---

## 3. Architecture

Single process when running `wails dev`:

```
┌─────────────────────────────────────────────────────────┐
│  Wails GUI (webview)  ←→  Go App  ←→  Core Engine      │
│                              ↕                          │
│                    HTTP API :8080 (same engine)         │
└──────────────────────────────┬──────────────────────────┘
                               ↓
                    LM Studio localhost:1234/v1
```

**Key rule:** GUI and every external driver share **one in-memory session**. IDE turns appear in the human chat window in real time via Wails events + WebSocket.

MCP mode (`go run ./cmd/harness -mode mcp`) is a **stdio proxy** to the HTTP API — it does not embed the engine. **Wails must be running** for MCP tools to work.

---

## 4. Implemented features

### 4.1 Core engine

| Feature | Status | Notes |
|---------|--------|-------|
| Config from `harness.toml` + env overrides | ✅ | `HARNESS_LMSTUDIO_URL`, `HARNESS_PORT`, etc. |
| YAML frontmatter for skills/agents | ✅ | Was TOML initially — fixed |
| Default agent from repo-root `AGENTS.md` | ✅ | Skips `INDEX.md` in agents dir |
| Rules concatenation | ✅ | `.md`, `.mdc`, `.txt` |
| System prompt assembly | ✅ | Persona + rules + compact skill **index** (not full bodies) |
| `load_skill` tool loop | ✅ | Injects skill body into system prompt mid-turn |
| `list_skills` / `load_skill` tools | ✅ | Optional `read_file`/`write_file`/`run_shell` gated off in config |
| File watcher hot reload | ✅ | Skills/agents/rules reload on disk change |
| Session manager (in-memory) | ✅ | Single active session; messages + metadata |
| JSONL run logging | ✅ | `harness/runs/YYYY-MM-DD.jsonl` per turn |
| Tool loop cap | ✅ | Max 8 steps |

### 4.2 LM Studio client

| Feature | Status | Notes |
|---------|--------|-------|
| `GET /v1/models` autodetect | ✅ | Never hardcode model ID |
| Chat completions + streaming | ✅ | |
| URL normalization | ✅ | `internal/lmstudio/url.go` |
| Empty message validation | ✅ | Rejects blank user messages |
| Always send `content` field | ✅ | Fixes LM Studio "undefined content" errors |

### 4.3 GUI (Wails)

| Feature | Status | Notes |
|---------|--------|-------|
| Chat transcript with streaming | ✅ | Wails `chat-stream` events |
| Settings panel (slide-over) | ✅ | Connection, model, agent, generation, paths, your name |
| Context editor (left panel) | ✅ | Skills/agents/rules tree + file editor + save |
| Collapsible sidebar | ✅ | Toggle rail always visible (☰ / ◀) |
| System prompt per turn | ✅ | Collapsible `<details>` on assistant messages |
| Participant display names | ✅ | See §4.6 |
| Top bar | ✅ | Connection status, model/agent chips, tokens/latency, Settings |

### 4.4 Live HTTP/WebSocket API (port 8080)

Starts automatically inside Wails app. Same engine as GUI.

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/health` | Health check |
| GET | `/models` | List LM Studio models |
| POST | `/connect` | Test connection + set endpoint |
| GET | `/skills`, `/agents`, `/rules` | List context |
| POST | `/reload` | Reload skills/agents/rules |
| GET | `/system-prompt?agent=` | Preview assembled prompt |
| GET | `/transcript` | Active session + `transcript_hash` |
| POST | `/message` | Send turn (requires `observer_name` for IDE) |
| GET | `/ws/transcript` | Live transcript WebSocket |
| GET | `/ws/chat` | Chat WebSocket (streaming) |
| POST | `/observers/register` | Register external agent name |
| GET | `/observers` | List observers + pending counts |
| GET | `/observers/catch-up?observer=&auto_ack=` | Incremental catch-up |
| POST | `/observers/ack` | Acknowledge without reading |

**WebSocket event types:** `transcript_snapshot`, `message_added`, `stream_start`, `stream_delta`, `stream_done`

GUI receives engine events via Wails `transcript-event` (bridged in `transcript_bridge.go`).

### 4.5 Observer / catch-up system

Designed so IDE agents don't poll full transcript or track hashes client-side.

**Workflow (required — see `rules/external-agent-sync.md`):**

1. `register_observer({ name? })` — custom name or auto-generated (`agent-k7m2x9`)
2. `catch_up({ observer_name, auto_ack: true })` — missed events since last ack
3. `send_message({ observer_name, message, model?, agent? })` — must register first
4. Repeat `catch_up` to stay in sync

**Behavior:**

- First register sets `last_ack_seq = 0` → first catch-up returns **full backlog**
- Catch-up log records **`message_added` only** (no duplicate `stream_start`/`stream_done`)
- Reconnecting with same name resumes queue; new name = new agent
- Observer state is **in-memory** (lost on app restart)

### 4.6 Participant naming (chat headers)

| Participant | How name is set | Display in chat |
|-------------|-----------------|-----------------|
| Human (GUI) | Settings → Your Name, or auto `human-xxxxxx` | User bubble: name only |
| External agent | `register_observer` custom or auto `agent-xxxxxx` | User bubble: agent name |
| Model reply | — | `Model • {model} · reply to {asker} · {latency} · {tokens}` |

Metadata fields on each message: `driver` (internal id), `participant_name` (display label).

### 4.7 MCP server

```bash
# With wails dev running on :8080:
go run ./cmd/harness -mode mcp -config harness.toml
# env: HARNESS_API_URL=http://localhost:8080
```

**Tools:** `register_observer`, `catch_up`, `ack_observer`, `send_message`, `list_observers`

MCP proxies to HTTP — does not run engine standalone.

### 4.8 CLI modes

| Mode | Command | Use |
|------|---------|-----|
| GUI (default) | `wails dev` from `harness/` | Desktop app + API on 8080 |
| Headless API | `go run ./cmd/harness -mode serve` | HTTP only (separate process — **don't run both on same port**) |
| MCP | `go run ./cmd/harness -mode mcp` | IDE stdio proxy |

---

## 5. UI layout (current)

```
┌─────────────────────────────────────────────────────────────┐
│ [● Connected]          model · agent        tokens · ⚙ Settings │
├────┬────────────────────────────────────────────────────────┤
│ ☰  │  Chat transcript (dominant)                             │
│    │  - User / external agent / model reply bubbles          │
│ ◀  │  - Collapsible system prompt on assistant turns        │
│panel│                                                         │
│    │                                                         │
├────┴────────────────────────────────────────────────────────┤
│ Driver: …    [ message input ]                    [ Send ]  │
└─────────────────────────────────────────────────────────────┘
```

- **Settings panel:** LM Studio endpoint, connect, model, agent, generation (read-only from toml), API URL, reload context, paths, your display name
- **No duplicate app title** in top bar (only in window chrome + welcome message)

---

## 6. Configuration reference

`harness/harness.toml`:

```toml
[server]
host = "localhost"
port = 8080

[lmstudio]
base_url = "http://localhost:1234/v1"

[paths]
skills_dir = "../skills"
agents_dir = "../agents"
rules_dir = "../rules"
runs_dir = "./runs"

[defaults]
agent = "default"
model = ""   # autodetect

[generation]
temperature = 0.7
max_tokens = 2048
top_p = 0.9
top_k = 40

[tools]
list_skills = true
load_skill = true
read_file = false
write_file = false
run_shell = false
```

---

## 7. Key source files (for agents)

| Area | Files |
|------|-------|
| Chat + tool loop | `internal/engine/chat.go` |
| Sessions | `internal/engine/session.go` |
| Observers | `internal/engine/observers.go`, `events.go` |
| Participant names | `internal/engine/names.go` |
| Prompt assembly | `internal/engine/prompt.go` |
| Loaders + frontmatter | `internal/engine/loaders.go`, `frontmatter.go` |
| HTTP API | `internal/api/server.go` |
| MCP proxy | `internal/mcp/server.go`, `remote.go` |
| Wails bindings | `app.go`, `transcript_bridge.go` |
| Frontend | `frontend/src/main.js`, `index.html`, `main.css` |
| Tests | `internal/engine/*_test.go`, `internal/api/server_test.go` |

Run tests: `cd harness && go test ./internal/...`

---

## 8. Known gotchas

1. **LM Studio is stateless** — harness sends full message history each turn; LM Studio does not store sessions.
2. **Session history is in-memory** — app reload wipes chat. JSONL runs log turns but don't restore GUI session.
3. **Port 8080 conflict** — don't run `wails dev` and `-mode serve` simultaneously on same port.
4. **MCP requires running GUI** — MCP is HTTP proxy only.
5. **POST /message requires `message` field** (not `content`) and registered `observer_name` for IDE agents.
6. **LM Studio single-request** — queue concurrent generations; don't fire parallel chat requests.
7. **Small model tool calling** — Gemma E4B may malform tool calls; loop feeds errors back and caps steps.
8. **Wails v2** — plan assumes v2 stable; v3 is alpha.
9. **`since_hash` on /transcript** — discussed but **not implemented**; use observer catch-up instead.
10. **Context growth** — system prompt + full history resent each turn; skill index stays compact until `load_skill`.

---

## 9. Not yet implemented (from PLAN.md)

Priority ideas for future work:

| Feature | PLAN milestone | Notes |
|---------|----------------|-------|
| **Session persistence** | — | Save/load sessions to disk; restore on restart |
| **Session fork UI** | M4 | `ForkSession()` exists in engine; no GUI/API exposure yet |
| **A/B compare view** | M4 | Run same prompt against two prompt versions side-by-side |
| **Regenerate / copy per turn** | M5 | UI actions on message bubbles |
| **Multiple sessions/tabs** | M5 | Engine supports multiple sessions; UI is single-session |
| **`since_hash` lightweight ping** | — | Incremental transcript check without full fetch |
| **MCP file tools** | M3 | `read_skill`, `write_skill`, `list_models` via MCP (only observer tools today) |
| **Optional read/write/shell tools** | M4 | Config exists; disabled by default |
| **Token budget meter** | M5 | Partial: system prompt ~token estimate in badge |
| **Export transcript** | M5 | — |
| **Session list API** | M3 | Create/switch/fork sessions over HTTP |
| **A2A protocol** | — | Deferred; MCP + HTTP sufficient for now |
| **Rename binary/module** | — | Display name is Nulqor; Go module still `harness` |
| **Wails v3 migration** | M5 | When stable |

---

## 10. IDE integration quick reference

### Cursor MCP config

```json
{
  "mcpServers": {
    "harness": {
      "command": "go",
      "args": ["run", "./cmd/harness", "-mode", "mcp", "-config", "harness.toml"],
      "cwd": "D:/source/skillTest/harness",
      "env": { "HARNESS_API_URL": "http://localhost:8080" }
    }
  }
}
```

### Minimal HTTP test

```powershell
# Register (auto name)
$reg = Invoke-RestMethod -Method Post -Uri "http://localhost:8080/observers/register" -Body "{}" -ContentType "application/json"

# Send
Invoke-RestMethod -Method Post -Uri "http://localhost:8080/message" -Body (@{
  message = "Hello"
  observer_name = $reg.name
} | ConvertTo-Json) -ContentType "application/json"

# Catch up
Invoke-RestMethod -Uri "http://localhost:8080/observers/catch-up?observer=$($reg.name)&auto_ack=true"
```

### Skills for IDE agents

- `skills/harness-live-transcript/SKILL.md` — observer workflow
- `skills/build-harness/SKILL.md` — build instructions from PLAN

---

## 11. Testing checklist

- [ ] LM Studio running with model loaded
- [ ] `wails dev` from `harness/`
- [ ] GUI connect in Settings → chat works
- [ ] Left panel toggle opens/closes
- [ ] Edit skill → save → hot reload
- [ ] `go test ./internal/...` passes
- [ ] HTTP `/health`, `/transcript`, observer register + catch-up + message
- [ ] IDE turn appears in GUI with correct participant headers
- [ ] Human name in Settings appears on human messages

---

## 12. Related documents

| File | Purpose |
|------|---------|
| `PLAN.md` | Full original spec, milestones, risks |
| `PROJECT_FEATURES.md` | Feature-level implementation notes (keep updated) |
| `harness/README.md` | Developer quick start |
| `rules/external-agent-sync.md` | Rule injected into every system prompt for external agents |
| `AGENTS.md` | Default persona body |

---

## 13. Change history (high level)

- Built Wails GUI + engine + LM Studio client + streaming
- Fixed YAML frontmatter, default agent loading, config path resolution
- Live HTTP/WebSocket API shared with GUI
- Observer catch-up with backlog on first register + deduped event log
- Participant naming (human + external agents, custom or auto-generated)
- Chat header fix: model replies show `Model · reply to {asker}`
- UI: Settings panel, sidebar toggle fix, renamed to **Nulqor**
- LM Studio payload fixes (empty message rejection, always include `content`)

---

*Last updated: 2026-05-24. Update this file when shipping major features or changing architecture.*
