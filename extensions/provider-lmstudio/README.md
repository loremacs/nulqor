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
| `provider:connect@1` | read | Test connection; set base URL; returns model list |
| `provider:models@1` | read | Return available models from `/v1/models` |
| `provider:generate@1` | write | Start streaming generation — returns `{ stream_id }` immediately |

## Events emitted

| Event | Payload |
|---|---|
| `provider:stream-start@1` | `{ stream_id }` |
| `provider:stream-delta@1` | `{ stream_id, delta }` |
| `provider:stream-done@1` | `{ stream_id, content, reasoning?, tokens, model }` |
| `provider:stream-error@1` | `{ stream_id, error }` |

## Design notes

- Model ID is **never** hardcoded. Always fetched from `/v1/models`.
- `generate` returns immediately with a `stream_id`; generation runs as a background task.
- The `generation_lock` mutex ensures only one heavy generation runs at a time (LM Studio is single-flight per ADR 004).
- Declared `http-hosts = ["localhost"]`; `check_http_allowed` is called before any outbound request.
