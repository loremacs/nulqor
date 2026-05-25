---
name: harness-live-transcript
description: Register as a harness observer and catch up on live chat without storing client-side state
triggers:
  - harness transcript
  - live conversation
  - read harness chat
  - harness observer
---

# Harness Live Transcript Access

## First: register a unique observer name

Each agent session must register once with either a custom name or an auto-generated name:

```
register_observer({ name: "My Windsurf Agent" })
```

Or let the harness assign a unique name:

```
register_observer({})
```

Examples of auto-generated names: `agent-k7m2x9`, `agent-a3f2b1`

- **Same name on reconnect** → resume your queue
- **New session** → register again (custom or auto name)
- **Your registered name** → shown on your messages and model replies in the GUI

## Catch up (preferred over full transcript)

```
catch_up({ observer_name: "agent-k7m2x9", auto_ack: true })
```

Returns only `message_added` events you missed since your last ack, then flushes your queue. The first catch-up after register includes the full existing backlog.

## Send a message

```
send_message({
  observer_name: "agent-k7m2x9",
  message: "...",
  model: "google/gemma-4-e4b",
  agent: "default"
})
```

Your observer name appears on both your message and the model reply in the GUI chat history.

## Prerequisites

- Harness running (`wails dev`) with API on port 8080
- MCP configured to proxy `http://localhost:8080` (see harness README)

## MCP config (Cursor)

```json
{
  "mcpServers": {
    "harness": {
      "command": "go",
      "args": ["run", "./cmd/harness", "-mode", "mcp", "-config", "harness.toml"],
      "cwd": "D:/source/skillTest/harness",
      "env": {
        "HARNESS_API_URL": "http://localhost:8080"
      }
    }
  }
}
```
