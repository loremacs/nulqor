# host

The canvas shell — transparent fullscreen desk with grid, draggable menu bar, and panel tiles.

## Responsibilities

- Fullscreen transparent overlay window (empty grid areas use CSS `pointer-events: none` inside the shell)
- Grid layout with snap-to-grid panel tiles
- Floating menu bar (Settings, Apps) — drag and dock to screen edges; **—** minimizes so you can use apps behind Nulqor
- Loads Panel extension UIs into tiles via `ui/panel.ts` mount contract

## Commands

| ID | Permission | Description |
|---|---|---|
| `canvas:status@1` | read | `{ "ready": true }` when canvas is mounted |
| `canvas:config@1` | read | Startup shell config, open panels, discovered Panel extensions |

## UI

| Path | Purpose |
|---|---|
| `ui/main.ts` | Shell entry (loaded from root `index.html`) |
| `ui/shell.ts` | Grid desk, menu bar, tile manager |
| `ui/panels.ts` | Dynamic panel loader registry |

## Events

- `canvas:ready@1` — emitted by core after all extensions activate

## Panel mount contract

Panel extensions expose `extensions/<id>/ui/panel.ts`:

```typescript
export function mount(container: HTMLElement): void { ... }
```

Register the panel in `host/ui/panels.ts` `PANEL_LOADERS`.

## Config (`nulqor.toml`)

```toml
open_panels = ["hello-world"]

[shell]
grid_cols = 12
grid_rows = 8
snap_enabled = true
show_grid = true
```

Shell layout persists in browser `localStorage` (`nulqor-shell-v5`):
- `shell` — grid size, snap, show grid (global desk settings)
- `panelLayouts` — last position/size per panel id (host-owned canvas state)
- `openPanelIds` — which panels are currently open
