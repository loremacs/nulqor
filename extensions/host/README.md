# host

The one built-in extension that owns the window shell. Responsible for:

- Providing the canvas surface that all panel extensions render into.
- Emitting `canvas:ready@1` after all extensions have been loaded and activated.

## Commands

| ID | Permission | Description |
|---|---|---|
| `canvas:status@1` | read | Returns `{ "ready": true }` when the canvas is mounted |

## Events emitted

- `canvas:ready@1` — emitted by the core (via `lib.rs`) after loader finishes

## Notes

This extension has no TypeScript UI of its own. It is always loaded first (no `requires`).
