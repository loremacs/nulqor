---
name: canvas-layout
description: >-
  Use when editing host canvas layout: grid mode, split/layout mode, sub-grids,
  saved profiles, panel drag, sash, or extensions/host/ui/shell.ts split-render
  canvas-profiles. Covers invariants, file map, and manual test checklist.
---

## Metadata

```text
version:       1.0.0
topics:        host, canvas, grid, split, sub-grid, layout, profiles, ui
platform:      all
script_policy: none
scope:         project-scoped
```

The `scripts/` directory is intentionally empty (instruction-only skill).

Decision context: [`docs/decisions/007-canvas-layout.md`](../../docs/decisions/007-canvas-layout.md).  
Deep invariants: [REFERENCE.md](REFERENCE.md).

---

## When to use

- Bug or feature in **grid desk**, **split layout**, **sub-grid (`#`)**, **saved profiles**, or **Layout menu**.
- Panel drag flicker, stuck panels after profile load, sash snap, z-order/stacking.
- Any edit under `extensions/host/ui/` touching layout (not Settings click-through only).

Do **not** use for Rust core, panel extension UIs (`extensions/<panel>/ui/`), or Phase 4+ persistence extensions.

---

## Contract

```text
when:         Before editing host canvas layout TypeScript or related CSS
inputs:       symptom or feature; mode (grid | split | both); profile involved?
outputs:      minimal diff in host/ui; tsc pass; manual checklist when behavior changes
side-effects: localStorage shape (nulqor-shell-v8) only if intentionally changed
validation:   npx tsc --noEmit; manual layout checklist in REFERENCE.md when UX changes
```

---

## File map (only touch what the task needs)

| File | Owns |
|------|------|
| `extensions/host/ui/shell.ts` | Mode router, grid tile drag/resize, stack order, persist, menus |
| `extensions/host/ui/split-render.ts` | Split DOM, cross-slot + sub-grid drag, incremental moves, sash |
| `extensions/host/ui/split-layout.ts` | Split tree model, `movePanelToLeaf`, presets, sub-grid state |
| `extensions/host/ui/canvas-profiles.ts` | Save/load/apply five profiles |
| `extensions/host/ui/grid.ts` | Cell geometry; pass `cssScope` for sub-grids |
| `extensions/host/ui/save-layout-dialog.ts` | Centered save-layout modal |
| `extensions/host/ui/style.css` | Panel z-index, split/sub-grid pointer-events |
| `extensions/host/ui/types.ts` | `TileLayout`, `STORAGE_KEY` (`nulqor-shell-v8`) |

**Boundary:** No `src-tauri/` changes for layout unless a decision record + human sign-off.

---

## Rules (do not violate)

1. **Incremental first** — Cross-slot and sub-grid moves must patch DOM via `applySplitPanelMoveInDom`. Full `renderCanvas()` remounts panel UIs (visible flicker).
2. **Live tree** — Split drag/persist uses `getTree: () => splitState.tree`, not a stale `opts.split` captured at render.
3. **DOM source leaf** — `resolvePanelSourceLeafId` trusts the DOM slot; pass `sourceLeafId` into `movePanelToLeaf`.
4. **Sub-grid coords** — Tile layouts are scoped to the sub-grid host. Never apply full-canvas `panelLayouts` / global `pixelLock` to sub-grid slots. Normalize on load/drag (`normalizeSubGridTile`).
5. **Sub-grid CSS vars** — `updateGridGeometry(..., cssScope: gridHost)` so sub-grid drags do not repaint unrelated panels.
6. **Intra sub-grid drag** — While pointer is inside the sub-grid host, reposition locally; cross-slot targeting only when pointer leaves that host.
7. **Grid stack** — Dragging raises panel (`panel-tile-dragging`, `z-index: 10`, `appendChild`, sync `openPanelIds` order).

Details and failure modes: [REFERENCE.md](REFERENCE.md).

---

## Workflow

1. Read decision 007 + [REFERENCE.md](REFERENCE.md) § relevant to the bug.
2. Read the file(s) in the map above — never edit from assumption.
3. Make a minimal diff; prefer fixing incremental path over adding full re-renders.
4. Verify:

   ```powershell
   npx tsc --noEmit
   ```

5. If behavior changed, run the **manual checklist** in REFERENCE.md (grid + split + profile load).
6. Update [`docs/PROJECT_FEATURES.md`](../../docs/PROJECT_FEATURES.md) canvas section only when shipped behavior changes.

---

## Verification

- [ ] Read 007 + REFERENCE sections for the touched area.
- [ ] `npx tsc --noEmit` passed or failures reported.
- [ ] Manual checklist run when drag, profile, or sash behavior changed.
- [ ] No core changes without decision record.
