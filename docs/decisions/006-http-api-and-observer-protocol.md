# 006 — HTTP API, WebSocket events, observer protocol, and MCP tool surface

Status: accepted

## Context

A working Go/Wails harness (living in `harness/` at the repo root) demonstrated the Phase 2 gateway
behavior on 2026-05-24. That implementation is now the reference for what the Phase 2 extensions
must reproduce in the Tauri/Rust architecture. This decision record extracts the entire proven
surface so the Go code does not need to be consulted again. Everything below was verified in a live
run (`harness/runs/2026-05-24.jsonl`) and covered by unit tests (`harness/internal/...`).

**The Go harness can be ignored after reading this document.** It proves the thesis; this ADR
preserves what it proved.

---

## Decision

Phase 2 extensions must implement the surface described in this document. Where the Go harness
made a choice that is compatible with the Rust/Tauri architecture, that choice is the accepted spec.
Do not deviate without a new decision record.

---

## 1. HTTP API surface (default port 8787, override via `NULQOR_PORT`)

All endpoints operate against the single active in-memory session through the shared engine. The
GUI and every external driver mutate the same session — this is the core demonstrated behavior.

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Health check → `{ "ok": true }` |
| GET | `/models` | List loaded models from LM Studio `GET /v1/models` |
| POST | `/connect` | Test connection + set LM Studio endpoint; body `{ "url": "..." }` |
| GET | `/skills` | List loaded skills (name + description) |
| GET | `/agents` | List loaded agents (name + description) |
| GET | `/rules` | List loaded rule files (filename + excerpt) |
| POST | `/reload` | Reload skills/agents/rules from disk |
| GET | `/system-prompt?agent=<name>` | Preview assembled system prompt for an agent |
| GET | `/transcript` | Active session messages + `transcript_hash` (see §5) |
| POST | `/message` | Send a user turn (see §3 for required shape) |
| GET | `/ws/transcript` | WebSocket — live transcript events (see §2) |
| GET | `/ws/chat` | WebSocket — streaming generation deltas (see §2) |
| POST | `/observers/register` | Register a named external agent (see §3) |
| GET | `/observers` | List all observers + pending event counts |
| GET | `/observers/catch-up?observer=<name>&auto_ack=<bool>` | Incremental event catch-up (see §3) |
| POST | `/observers/ack` | Flush observer queue without reading |

The HTTP API is how the GUI operates internally. It must be available on the same port while the
Tauri app is running. The GUI uses it; external IDE agents, scripts, and the MCP proxy all use it.

---

## 2. WebSocket event types

Both WebSocket paths (`/ws/transcript` and `/ws/chat`) emit the same typed event envelope. Every
message is JSON:

```json
{ "type": "<EventType>", ...fields }
```

| Type | When emitted | Required fields |
|------|-------------|-----------------|
| `transcript_snapshot` | On WebSocket connect | `messages: [Message]` |
| `message_added` | Any turn added (user or assistant) | `message: Message` |
| `stream_start` | Model begins streaming | `stream_id: string` |
| `stream_delta` | Each token chunk | `stream_id: string, delta: string` |
| `stream_done` | Generation complete | `stream_id: string, message: Message` |

The Tauri GUI receives these same events via the IPC bridge (`transcript:message-added@1` etc.)
rather than WebSocket, but the payload shapes are identical.

---

## 3. Observer / catch-up protocol (the IDE-driver contract)

This is the most critical design in Phase 2. It enables IDE agents to drive the session without
polling the full transcript or maintaining client-side state. All behaviors below were verified
in tests (`internal/engine/observers_backlog_test.go`, `internal/api/server_test.go`).

### Registration

`POST /observers/register`
Body: `{ "name": "My-Agent" }` for custom name, or `{ "name": "" }` for auto-generated.

Rules:
- Auto-generated names: `agent-<6-char-random>` (e.g. `agent-k7m2x9`).
- Names: 3–32 chars, start with a letter or digit, may use letters, digits, `_`, `-`, space.
- **Duplicate name (case-insensitive): returns the existing observer — not an error.** This makes
  reconnection idempotent.
- Registration sets `last_ack_seq = 0`. The **first `catch_up` therefore returns the full existing
  transcript backlog**, not just events since registration. This is intentional: a new IDE agent
  joining a running session immediately sees the full conversation history.

Response: `{ "name": "agent-k7m2x9", "last_ack_seq": 0, "pending_count": N }`

### Catch-up

`GET /observers/catch-up?observer=<name>&auto_ack=<true|false>`

- Returns events with `seq > last_ack_seq` up to current head.
- `auto_ack=true` advances `last_ack_seq` to the last returned seq, flushing the queue.
- `auto_ack=false` (default): queue position unchanged; same events returned on next call.
- **Catch-up log contains `message_added` events only.** `stream_start` and `stream_done` are
  excluded to prevent duplicate representation. An agent reading catch-up sees completed turns, not
  in-progress stream fragments. (Use the WebSocket for live streaming deltas instead.)

Response: `{ "events": [{ "seq": 7, "event": { "type": "message_added", "message": {...} } }] }`

### Acknowledge (flush without reading)

`POST /observers/ack` body `{ "name": "<name>" }` — advances `last_ack_seq` to current head.

### Sending as an observer

`POST /message` body:
```json
{
  "message": "text content",
  "observer_name": "<registered-name>",
  "model": "<optional, overrides default>",
  "agent": "<optional, overrides active agent>"
}
```

- `observer_name` is **required** for external/IDE callers. Missing or unregistered name → `400`.
- The observer's display name appears on both the user turn bubble and the `reply to <name>` label
  on the following assistant turn.
- Human GUI turns bypass this field; they use a separate Wails binding with the human's display name.

### Observer state

- In-memory only through Phase 2 and 3. Persistence is Phase 4.
- App restart wipes all observers. Stale observers (never reconnected) accumulate events silently
  without causing errors.

---

## 4. Participant naming

Every turn in the transcript carries two fields: `driver` (internal routing id) and
`participant_name` (the display label shown in the chat UI).

| Caller | `participant_name` | `driver` |
|--------|--------------------|---------|
| Human (GUI) | Settings → "Your Name"; auto `human-<6-char>` if not set | same as participant_name |
| External IDE agent | Name passed to `register_observer` | same as participant_name |
| Model (assistant) | Model display label | `"assistant"` |

**Chat UI display rule for model turns:**
`Model · <model-id> · reply to <asker-name> · <latency>ms · <tokens> tok`

The `reply to` field uses the `participant_name` of the user turn that triggered generation.

---

## 5. Message schema

```json
{
  "id": "uuid-or-slug",
  "role": "user | assistant | tool",
  "content": "...",
  "timestamp": "2026-05-24T00:11:04-07:00",
  "model": "google/gemma-4-e4b",
  "latency_ms": 1564,
  "tokens": 353,
  "driver": "human-abc123 | agent-k7m2x9 | assistant",
  "participant_name": "Alice | agent-k7m2x9 | Model",
  "reasoning": "..."
}
```

`reasoning` is optional. LM Studio returns it in `reasoning_content` for models that surface inner
chain-of-thought. When present, the UI renders it in a collapsible "Thinking" block before the
reply. The run log entry (`§8`) also captures it.

`transcript_hash` on the `/transcript` response is a content hash of the current message list. It
lets a caller detect whether the transcript has changed since a prior fetch without comparing full
message arrays.

---

## 6. System prompt assembly

Built once per turn from three sources, assembled in this order:

1. **Agent persona** — full body of `AGENTS.md` (default agent) or `agents/<name>.md`.
2. **Rules** — all files under `rules/` with extensions `.md`, `.mdc`, `.txt`, concatenated in
   alphabetical order. Files named `INDEX.md` are skipped.
3. **Compact skill index** — one line per skill: `- **<name>**: <description>`. Full skill bodies
   are **not** included here. They are injected into the system prompt on demand via `load_skill`.

The assembled system prompt is stored on the assistant turn in the transcript (for inspection) and
in the JSONL run log entry.

**Harness token cost** (system prompt size) must be reported per turn as a budget line item in the
UI and in the run log. This is a quality gate (`DESIGN.md §13`).

---

## 7. Skill format and tool loop

**Skill file structure** (`skills/<name>/SKILL.md`):

```
---
name: <name>
description: <what + when; shown in compact index>
---

## Metadata
(version, topics, platform, script_policy, scope — in body block, not frontmatter)

## When to use / ## Contract / ## Steps / ## Verification
```

Structural compliance: `skills/audit-skill/scripts/audit.ps1` (all skills or `-SkillName`).

**Built-in tools (always registered by the skill runner extension):**

| Tool | Behavior |
|------|----------|
| `list_skills` | Returns the compact skill index (name + description list) |
| `load_skill(name)` | Fetches the full body of the named skill and injects it into the active system prompt for this turn |

**Optional tools (off by default — configurable per §10):** `read_file`, `write_file`, `run_shell`.

**Tool loop rules:**
- Hard cap: 8 tool calls per turn. Configurable. Never infinite.
- Malformed tool call JSON: catch the parse error, return it as a tool result message, continue
  the loop. Never crash the turn; never silently succeed.
- Log each tool call (name, input, output) in the turn's JSONL entry.

---

## 8. JSONL run log schema

Append one JSON line per completed assistant turn to `runs/YYYY-MM-DD.jsonl`:

```json
{
  "agent": "default",
  "driver": "human | ide | <observer-name>",
  "latency_ms": 1564,
  "model": "google/gemma-4-e4b",
  "reply": "...",
  "reasoning": "...",
  "system_prompt": "...",
  "timestamp": "2026-05-24T00:11:04-07:00",
  "tokens": 353,
  "user_message": "..."
}
```

`reasoning` is omitted when the model returns none. All other fields are always present. The log
is append-only and human-readable. It is the primary artifact for judging whether the Subject is
getting better or worse over time — protect it.

---

## 9. MCP tool surface (stdio proxy to HTTP API)

The MCP server is a thin stdio proxy to the HTTP API. It does **not** embed the engine. The Tauri
app must be running for MCP tools to work.

Expose exactly these tools, mapping to HTTP endpoints as shown:

| Tool | HTTP mapping |
|------|-------------|
| `register_observer(name?)` | `POST /observers/register` |
| `catch_up(observer_name, auto_ack?)` | `GET /observers/catch-up` |
| `ack_observer(observer_name)` | `POST /observers/ack` |
| `send_message(message, observer_name, model?, agent?)` | `POST /message` |
| `list_observers()` | `GET /observers` |

MCP transport: stdio for IDE config (Cursor/Windsurf). Offer streamable-HTTP as a second transport.

Cursor MCP config reference:
```json
{
  "mcpServers": {
    "nulqor": {
      "command": "<path-to-nulqor-mcp-binary>",
      "args": ["-mode", "mcp"],
      "env": { "NULQOR_API_URL": "http://localhost:8787" }
    }
  }
}
```

---

## 10. Config reference (extension.toml / harness.toml equivalent)

The HTTP API, transcript, and skill runner extensions should be configurable via their
`extension.toml` config blocks or a top-level `harness.toml` at the workspace root:

```toml
[server]
host = "localhost"
port = 8787

[lmstudio]
base_url = "http://localhost:1234/v1"

[paths]
skills_dir = "../skills"
agents_dir = "../agents"
rules_dir  = "../rules"
runs_dir   = "./runs"

[defaults]
agent = "default"
model = ""       # empty = autodetect from /v1/models on connect

[generation]
temperature = 0.7
max_tokens  = 2048
top_p       = 0.9
top_k       = 40

[tools]
list_skills = true
load_skill  = true
read_file   = false
write_file  = false
run_shell   = false
```

All paths resolve relative to the config file's directory. Environment variable overrides must be
supported (e.g. `NULQOR_PORT`, `NULQOR_LMSTUDIO_URL`) for headless/CI use.

---

## 11. Known gotchas (from live operation — do not rediscover these)

1. **LM Studio is stateless.** Send the full message history each turn. LM Studio does not store
   sessions server-side.
2. **Session history is in-memory through Phase 3.** App restart wipes the chat transcript.
   JSONL run logs persist turns but do not restore the GUI session. Persistence is Phase 4 work.
3. **LM Studio serves one heavy generation at a time.** Queue concurrent requests in the provider
   extension (ADR 004). Never fire parallel completions and expect parallelism.
4. **Small model tool calling is imperfect.** Gemma E4B sometimes malforms tool call JSON. The
   loop must catch parse errors, feed them back as tool results, and cap at the step limit. Never
   let a bad tool call crash the turn.
5. **MCP requires a running app.** The MCP server is a proxy; it has no engine of its own.
6. **`POST /message` requires `observer_name`** for any external caller. Human GUI turns bypass
   this via a separate Wails binding. Missing or unregistered `observer_name` → 400.
7. **Model ID is never hardcoded.** Always fetch from `/v1/models` on connect. The loaded model
   changes; assuming a fixed ID breaks silently.
8. **Context grows with history.** The full message history is resent to LM Studio each turn
   (LM Studio is stateless — see point 1). The skill index stays compact until `load_skill` is
   called. Watch the token budget.
9. **Catch-up log is `message_added` only.** `stream_start` and `stream_done` are excluded from
   the catch-up queue. They appear only on the WebSocket. An IDE agent reading catch-up gets
   completed turns, not half-streamed ones.
10. **YAML frontmatter, not TOML.** Skill and agent files use YAML between `---` markers. An
    early implementation used TOML by mistake; YAML is correct and proven.

---

## Consequences

- Phase 2 has a fully specified, implementation-ready surface without consulting the Go harness.
- An agent implementing Phase 2 has a concrete acceptance target: all endpoints in §1 must work,
  observer behavior must match §3 exactly, and the JSONL schema in §8 is the run log format.
- The Go harness (`harness/`) can be ignored as source material. This ADR supersedes it.
