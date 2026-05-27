# 007 — Canvas layout (grid + split, saved profiles)

**Status:** Accepted  
**Date:** 2026-05-24

## Context

The host shell (`extensions/host/`) renders the transparent canvas: menu bar, panel tiles, click-through, and grid desk. Users need two layout engines (free grid vs split-pane presets), up to five saved canvas snapshots, edit mode (with click-through suspended), and optional sub-grids inside split slots.

## Decision

1. **All canvas layout UI and logic live in the host extension** — TypeScript under `extensions/host/ui/`. No core (`src-tauri`) changes. Rust continues to expose only `canvas:config@1` / `canvas:status@1`.

2. **Third menu: Layout** — Peer to Settings (window prefs) and Apps (panel toggles). Grid tuning (cell size, snap, show grid) moves from Settings to Layout.

3. **Two modes**
   - **Grid mode** — existing tile desk (`grid.ts`).
   - **Layout mode (split)** — N-ary split tree (`split-layout.ts`), VS Code–style presets, draggable sashes.

4. **Five saved profiles** — `CanvasProfile` in `localStorage` (`nulqor-shell-v8`). Each stores mode, open panels, and mode-specific state. Built-in presets are templates, not saved slots.

5. **Edit mode** — Enter from Layout menu; suspends click-through; split slots show split/merge/sub-grid controls; grid mode keeps tile drag/resize.

6. **Sub-grids** — Optional per split leaf; reuses `grid.ts` scoped to the slot container.

## Persistence shape

```ts
PersistedShellState {
  canvasMode: "grid" | "split"
  split?: SplitCanvasState
  canvasProfiles: (CanvasProfile | null)[5]
  activeProfileId: string | null
  layoutEditing: boolean
  // existing: menuDock, shell, panelLayouts, openPanelIds, windowFrame
}
```

## Files

| Path | Role |
|------|------|
| `extensions/host/ui/shell.ts` | Menu wiring, mode router, persist |
| `extensions/host/ui/split-layout.ts` | Tree model, presets, sash math |
| `extensions/host/ui/split-render.ts` | Split DOM, sub-grid, edit chrome |
| `extensions/host/ui/canvas-profiles.ts` | Save/load/apply profiles |
| `extensions/host/ui/grid.ts` | Grid engine (root + sub-grid scope) |
| `extensions/host/ui/types.ts` | Shared types, `STORAGE_KEY` v8 |

## Consequences

- Panel extensions unchanged; host owns all layout chrome.
- Future layout APIs (e.g. `canvas:layout-saved@1` event) can be added as host commands without core growth until explicitly approved.
