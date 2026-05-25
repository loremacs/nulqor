# Hello World

Minimal panel extension — fills the main window with "Hello World".

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest |
| `src/lib.rs` | Panel backend (subscribes to `canvas:ready@1`) |
| `ui/` | Panel UI — `panel.ts` with `mount()` for tiles; optional standalone `main.ts` |

Requires `host`. Enable via root `nulqor.toml`.
