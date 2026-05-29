# provider-lmstudio

Slotted `provider`/`lmstudio` extension. Connects to LM Studio's OpenAI-compatible API and
owns the single-flight request queue (one heavy generation at a time).

## Capability provided

| Slot | Instance | Contract |
|---|---|---|
| `provider` | `lmstudio` | `provider@1` |

## Commands

| ID | Permission | Description |
|---|---|---|
| `provider:connect@1` | read | Ping server and **load** `{ model }` — sets `connected: true` when ready |
| `provider:disconnect@1` | read | Unload Nulqor-owned models; clear session |
| `provider:models@1` | read | Cached catalog; `{ refresh: true, url? }` fetches options only (no load) |
| `provider:select-model@1` | read | Load/select model; tracks Nulqor-owned instances for safe unload |
| `provider:stop-model@1` | read | Unload active model **only if this Nulqor process loaded it** |
| `provider:generate@1` | write | Start streaming generation — returns `{ stream_id }` immediately |

## Events emitted

| Event | Payload |
|---|---|
| `provider:stream-start@1` | `{ stream_id }` |
| `provider:stream-delta@1` | `{ stream_id, delta }` |
| `provider:stream-done@1` | `{ stream_id, content, reasoning?, tokens, model }` |
| `provider:stream-error@1` | `{ stream_id, error }` |

## Design notes

- Model ID is **never** hardcoded.
- **Connect** only verifies the server is reachable — it does **not** fetch models.
- **Fetch models** uses `provider:models@1 { refresh: true }` — probes `GET /v1/models`, then falls back to `GET /api/v1/models`.
- **`provider:select-model@1`** loads via `POST /api/v1/models/load` when the model is not already loaded. If LM Studio already has the model loaded (e.g. another app), Nulqor adopts it without tracking it for unload.
- **Nulqor-owned tracking:** each process keeps an in-memory list of `{ model, instance_id }` for loads it initiated. **Stop** and **Disconnect** call `POST /api/v1/models/unload` only for those instances — never for externally loaded models.
- **`nulqor_loaded_active`** in `models@1` indicates whether the active model can be stopped safely from this window.
- `generate` returns immediately with a `stream_id`; generation runs as a background task.
- The `generation_lock` mutex ensures only one heavy generation runs at a time (LM Studio is single-flight per ADR 004).
- Declared `http-hosts = ["localhost"]`; `check_http_allowed` is called before any outbound request.

## Chat panel flow

1. **Fetch models** — list catalog into dropdown (no load)
2. **Select model** from dropdown
3. **Connect** — load/spin up the selected model
4. **Stop model** — unload if Nulqor loaded it
5. **Disconnect** — unload all Nulqor-owned models
