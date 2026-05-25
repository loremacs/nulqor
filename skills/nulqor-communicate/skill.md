# nulqor-communicate

**Type:** Skill  
**Status:** Active

## Purpose

Talk to a **running** Nulqor desktop app. Covers every external communication path in the
current Phase 2–3 setup: HTTP API, MCP stdio server, in-app MCP bridge commands, WebSockets,
and Tauri IPC. Run this skill's script instead of guessing endpoints.

**Prerequisite:** App running (`npm start` from repo root). For model replies, LM Studio at
`http://localhost:1234/v1` with a loaded model.

## Contract

```yaml
name: nulqor-communicate
version: 0.1.0
inputs:
  - action: health | connect | models | register | send | transcript | catch-up | ack | observers
  - message: required for send
  - observer_name: default cursor-agent
  - base_url: default NULQOR_API_URL or http://127.0.0.1:8080
outputs:
  - api_json: endpoint response body
  - assistant_reply: for send, last assistant content when -WaitSeconds succeeds
tool_loop_cap: 5
```

## Quick start (HTTP script)

```powershell
# Health check
skills/nulqor-communicate/scripts/chat.ps1 -Action health

# Optional: point provider at LM Studio (default URL usually already works)
skills/nulqor-communicate/scripts/chat.ps1 -Action connect

# Send a test message and wait for assistant reply
skills/nulqor-communicate/scripts/chat.ps1 -Action send `
  -Message "What is today's date?" -ObserverName "my-agent"
```

## Communication surfaces (current setup)

### 1. HTTP API (primary) — port **8080**

Base URL: `NULQOR_API_URL` env var or `http://127.0.0.1:8080`.

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | `{ "ok": true }` |
| POST | `/connect` | Set LM Studio URL `{ "url": "http://localhost:1234/v1" }` |
| GET | `/models` | List models via provider |
| GET | `/transcript` | Full session + `transcript_hash` |
| POST | `/message` | Send user turn + start generation (requires `observer_name`) |
| POST | `/observers/register` | Register IDE agent `{ "name": "..." }` (empty name = auto) |
| GET | `/observers` | List observers |
| GET | `/observers/catch-up?observer=<n>&auto_ack=<bool>` | Incremental `message_added` events |
| POST | `/observers/ack` | `{ "name": "<observer>" }` flush queue |
| GET | `/ws/transcript` | WebSocket — live transcript events |
| GET | `/ws/chat` | WebSocket — streaming deltas (same envelope as transcript WS) |

**External caller flow (decisions/006 §3):**

1. `POST /observers/register`
2. `POST /message` with `{ "message", "observer_name", "model?", "agent?" }`
3. Poll `GET /transcript` or use `catch-up` / WebSocket for replies

**Not yet exposed over HTTP** (spec only): `/skills`, `/agents`, `/rules`, `/reload`, `/system-prompt`.
Use context-editor commands via Tauri IPC or extend http-api in a future phase.

### 2. MCP stdio server — `tools/mcp-server`

Cursor/Windsurf connect via `.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "nulqor": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "tools/mcp-server/Cargo.toml", "--quiet", "--"],
      "env": { "NULQOR_API_URL": "http://localhost:8080" }
    }
  }
}
```

**Tools (proxy to HTTP above):**

| MCP tool | HTTP equivalent |
|---|---|
| `register_observer` | `POST /observers/register` |
| `catch_up` | `GET /observers/catch-up` |
| `ack_observer` | `POST /observers/ack` |
| `send_message` | `POST /message` |
| `list_observers` | `GET /observers` |

### 3. MCP bridge commands (in-process)

Registered by `extensions/mcp-bridge/` — same five operations as HTTP, callable as core commands:

| Command | Input |
|---|---|
| `mcp-bridge:register-observer@1` | `{ "name"? }` |
| `mcp-bridge:catch-up@1` | `{ "observer_name", "auto_ack"? }` |
| `mcp-bridge:ack-observer@1` | `{ "name" }` |
| `mcp-bridge:send-message@1` | `{ "message", "observer_name", "model?", "agent?" }` |
| `mcp-bridge:list-observers@1` | `{}` |

Requires `NULQOR_API_URL` (default `http://localhost:8080`). Used when invoking via Tauri IPC.

### 4. Tauri IPC (GUI / frontend)

| Invoke | Purpose |
|---|---|
| `core_invoke` | Call any registered command `{ namespace, action, version, input }` |
| `core_list_commands` | List available commands |

Frontend chat panel uses these + bus events (`transcript:message-added@1`, `provider:stream-*`).

### 5. Run logs

`run-logger` extension appends each turn to `runs/YYYY-MM-DD.jsonl` (gitignored).

## Script reference

```powershell
skills/nulqor-communicate/scripts/chat.ps1 -Action <name> [options]
```

| Action | Options |
|---|---|
| `health` | |
| `connect` | `-Url` (default localhost:1234/v1) |
| `models` | |
| `register` | `-ObserverName` |
| `send` | `-Message` (required), `-ObserverName`, `-Model`, `-Agent`, `-WaitSeconds` |
| `transcript` | |
| `catch-up` | `-ObserverName`, `-AutoAck` |
| `ack` | `-ObserverName` |
| `observers` | |

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Connection refused on 8080 | App not running | `npm start` |
| `observer not registered` 400 | Skipped register | `-Action register` first |
| No assistant reply | LM Studio down / no model | Start LM Studio, load model |
| `/connect` failed / hung (historical) | `block_on` inside async HTTP worker | Fixed: `Runtime::block_on_compat` uses `block_in_place` — rebuild and restart app |

## After using

No skill audit required unless you edit this skill. After changing HTTP routes, update this file
and `docs/decisions/006-http-api-and-observer-protocol.md`.
