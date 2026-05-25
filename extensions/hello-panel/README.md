# hello-panel

Phase 1 end-to-end proof extension. Demonstrates the full extension lifecycle:

1. Loader discovers `extension.toml`, lints it, and activates in dependency order (`host` first).
2. `activate()` registers `hello:ping@1` via the command registry.
3. `activate()` subscribes to `canvas:ready@1` via the event bus.
4. The TypeScript frontend invokes `hello:ping@1` via Tauri IPC.
5. The bus fires `canvas:ready@1` and hello-panel logs receipt.

## Commands

| ID | Permission | Description |
|---|---|---|
| `hello:ping@1` | read | Returns `{ "pong": true, "source": "hello-panel" }` |

## Subscriptions

| Pattern | Purpose |
|---|---|
| `canvas:ready@1` | Trigger panel mount after canvas is available |

## Dependencies

Requires `host` (canvas must exist before the panel mounts).
