# External Agent Sync

External agents (Cursor, Windsurf, scripts, etc.) must follow this workflow when talking to Nulqor.

## Session identity

- Each external agent **session** registers with either a **custom name** or an **auto-generated name** (for example `agent-k7m2x9`).
- Names are **3–32 characters**, start with a letter or number, and may use letters, numbers, spaces, `_`, `-`.
- **Reconnecting with the same name** resumes your catch-up queue.
- **A new agent session uses a new name** — treat it like a different agent.
- **First `catch_up` after register** returns the full existing transcript backlog, then only new events after that.

## Required workflow

1. **`register_observer(name?)`** — once per agent session. Omit `name` or pass `""` for a unique auto-generated name.
2. **`catch_up(observer_name, auto_ack=true)`** — fetch missed transcript events and flush your queue.
3. **`send_message(..., observer_name=...)`** — send as that observer; your name appears on user and assistant turns in the GUI.
4. Repeat **`catch_up`** whenever you need to see what happened since your last read.

## Human GUI users

Humans set their own display name in **Settings → Your Name**, or click **Generate random name**. That label appears on their messages in the shared chat.

## Do not

- Reuse another agent's observer name unless you are reconnecting the same session.
- Assume full transcript fetches are required — use catch-up instead.
- Store transcript state client-side — the harness server tracks your queue per observer name.

## MCP tools

| Tool | Purpose |
|---|---|
| `register_observer` | Claim a custom or auto-generated unique name |
| `catch_up` | Read + optionally flush missed events |
| `ack_observer` | Flush without reading |
| `send_message` | Send a turn as registered observer |
| `list_observers` | See all registered agents and pending counts |

## HTTP equivalents

- `POST /observers/register` `{ "name": "My Agent" }` or `{ "name": "" }` for auto-generated
- `GET /observers/catch-up?observer=agent-k7m2x9&auto_ack=true`
- `POST /observers/ack` `{ "name": "agent-k7m2x9" }`
- `POST /message` with `"observer_name": "agent-k7m2x9"` (must register first; name is shown in chat history)
