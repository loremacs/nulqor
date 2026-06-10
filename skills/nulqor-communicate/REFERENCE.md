# Reference — nulqor-communicate

Loaded on demand. HTTP routes, MCP tools, bridge commands, and troubleshooting.

---

## HTTP API — port 8787

Base URL: `NULQOR_API_URL` or `http://127.0.0.1:8787`.

| Method | Path | Purpose |
|---|---|---|
| GET | `/health` | `{ "ok": true }` — app only; does not check LM Studio |
| GET | `/models` | `{ "models": [...], "active": "string|null" }` — `active` set after Connect |
| POST | `/connect` | Set LM Studio URL; sets active model |
| GET | `/transcript` | Full session + `transcript_hash` |
| POST | `/message` | Send user turn (requires `observer_name`) |
| POST | `/observers/register` | Register IDE agent |
| GET | `/observers` | List observers |
| GET | `/observers/catch-up` | Incremental events |
| POST | `/observers/ack` | Flush queue |
| GET | `/ws/transcript` | WebSocket transcript |
| GET | `/ws/chat` | WebSocket streaming |

**External flow:** register → message → poll transcript / catch-up / WebSocket.

**Not on HTTP yet:** `/skills`, `/agents`, `/rules`, `/reload`, `/system-prompt`.

---

## MCP stdio — `tools/mcp-server`

Connect via `.cursor/mcp.json` with `cargo run --manifest-path tools/mcp-server/Cargo.toml`.

| MCP tool | HTTP equivalent |
|---|---|
| `register_observer` | POST `/observers/register` |
| `catch_up` | GET `/observers/catch-up` |
| `ack_observer` | POST `/observers/ack` |
| `send_message` | POST `/message` |
| `list_observers` | GET `/observers` |

---

## MCP bridge commands (in-process)

| Command | Purpose |
|---|---|
| `mcp-bridge:register-observer@1` | Register observer |
| `mcp-bridge:catch-up@1` | Incremental events |
| `mcp-bridge:ack-observer@1` | Ack queue |
| `mcp-bridge:send-message@1` | Send message |
| `mcp-bridge:list-observers@1` | List observers |

---

## Tauri IPC

| Invoke | Purpose |
|---|---|
| `core_invoke` | Call registered command |
| `core_list_commands` | List commands |

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| Connection refused :8787 | `npm start` |
| `ready` → `no_active_model` | Click **Disconnect** then **Connect** in chat-panel, or `chat.ps1 -Action connect` |
| `ready` → `provider_unreachable` | Start LM Studio; load a model |
| `ready` → `no_models_loaded` | Load a model in LM Studio, then `-Action connect` |
| observer not registered | `-Action register` first (send auto-registers) |
| No assistant reply after send | Re-run `-Action ready`; check LM Studio logs |
| `/connect` hung (historical) | Rebuild app (`block_on_compat` fix) |

**chat.ps1 actions:** `ready` = app + provider preflight; `health` = app only.

---

## Run logs

`run-logger` writes `runs/YYYY-MM-DD.jsonl` (gitignored).
