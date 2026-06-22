# Provider Router

Routes public `provider:*@1` commands to the active local backend. The active instance
comes from `nulqor.toml` → `active_provider`; backends register under their own instance
namespace (`lmstudio:*`, `ollama:*`, `llamacpp:*`).

## Commands

| Command | Purpose |
|---|---|
| `provider:connect@1` / `provider:disconnect@1` | Connect/disconnect the active backend |
| `provider:models@1` / `provider:loaded-models@1` | List available / currently loaded models |
| `provider:select-model@1` / `provider:stop-model@1` / `provider:unload-model@1` | Manage the loaded model |
| `provider:generate@1` | Stream a completion (returns `stream_id`) |
| `provider:info@1` / `provider:set-active@1` | Inspect / switch the active provider |

## Notes

- Shared HTTP/OpenAI helpers live in `extensions/provider-common/` (shared module, not a loadable extension).
- Catalogs other backends, so it is exempt from the audit's port-collision check.

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest + command declarations |
| `src/lib.rs` | Rust service implementation |
