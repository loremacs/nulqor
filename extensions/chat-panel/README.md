# Chat Panel

Dominant chat UI with streaming, reasoning blocks, and token stats.

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest |
| `src/lib.rs` | Panel extension (event subscriptions, backend wiring) |
| `ui/main.ts` | TypeScript chat UI (Tauri entry via root `index.html`) |
| `ui/style.css` | Panel styles |

Commands and events: see `docs/decisions/006-http-api-and-observer-protocol.md`.
