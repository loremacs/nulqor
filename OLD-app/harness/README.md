# Nulqor

A desktop workbench for iterating on skills, agent personas, and rules against a local model served by LM Studio.

## Quick start

1. Start LM Studio and load a model (Gemma 4 recommended).
2. From `harness/`:

```bash
# GUI
wails dev

# Or run the built binary
./build/bin/harness-dev.exe

# Headless HTTP API (for IDE/scripts)
go run ./cmd/harness -config harness.toml -mode serve
```

3. Connect in the UI, pick a model, and chat.

## Content layout

The harness reads workspace content from the repo root via `harness.toml`:

| Path | Purpose |
|---|---|
| `../skills/<name>/SKILL.md` | Skills with YAML frontmatter |
| `../agents/*.md` | Agent personas |
| `../rules/*.{md,mdc,txt}` | Always-on rules |
| `AGENTS.md` (repo root) | Default agent persona |

Edit these files in Cursor/Windsurf or in the harness left panel. Changes hot-reload automatically.

## HTTP API

When running `wails dev`, the GUI **also starts the live API** on port 8080 using the same session. Cursor or any IDE agent can read the conversation as it happens.

| Method | URL | Use |
|---|---|---|
| Snapshot | `GET http://localhost:8080/transcript` | Full active session right now |
| Live feed | `ws://localhost:8080/ws/transcript` | Push events on every message/stream chunk |
| Send as IDE | `POST http://localhost:8080/message` | Inject a turn (`driver: ide`) into the same window |

Event types on the WebSocket: `transcript_snapshot`, `message_added`, `stream_start`, `stream_delta`, `stream_done`.

### External agent observers

Each IDE agent session registers a **unique name** and uses server-side catch-up queues (no client-side hash tracking):

| Method | URL | Use |
|---|---|---|
| Register | `POST /observers/register` `{ "name": "Cursor-abc" }` | Claim a unique observer name |
| Catch up | `GET /observers/catch-up?observer=Cursor-abc&auto_ack=true` | Fetch missed events + flush queue |
| Ack | `POST /observers/ack` `{ "name": "Cursor-abc" }` | Flush without reading |
| List | `GET /observers` | All observers + pending counts |

See `../rules/external-agent-sync.md` for the required agent workflow.

### MCP (Cursor / Windsurf)

Run with the GUI up (`wails dev`), then add MCP:

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

Tools: `register_observer`, `catch_up`, `ack_observer`, `send_message`, `list_observers`.

## What's implemented

- Wails GUI with connection bar, chat, and context editor
- **Live transcript API** — GUI and IDE share one session on port 8080
- LM Studio client with model autodetect and streaming
- YAML frontmatter parsing for skills/agents
- System prompt assembly (agent + rules + skill index)
- Hot reload via file watcher
- Built-in `list_skills` / `load_skill` tool loop
- Session history and JSONL run logging in `runs/`
- HTTP/WebSocket API for IDE integration

## Still TODO

- MCP server (stdio) for Cursor/Windsurf native MCP integration
- Session fork UI and A/B compare view
- Optional filesystem/shell tools (gated in config)

See `../PLAN.md` for the full spec.
