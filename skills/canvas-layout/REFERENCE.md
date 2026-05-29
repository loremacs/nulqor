# Canvas layout — reference

Agent reference for `extensions/host/ui/` layout. Spec: `docs/decisions/007-canvas-layout.md`.

---

## Modes

| Mode | Engine | Persistence |
|------|--------|-------------|
| **Grid** | `grid.ts` + `shell.ts` tile drag | `panelLayouts`, `openPanelIds` in `localStorage` |
| **Layout (split)** | `split-layout.ts` tree + `split-render.ts` DOM | `splitState.tree` + optional `canvasProfiles[5]` |

**Edit mode** (`layoutEditing`): suspends click-through; split slots show split/merge/`#` bar.

---

## Window mode vs click-through

Spec detail: `docs/PROJECT_FEATURES.md` §0.7. Code: `shell.ts` (`applyWindowModePolicy`), `click-through.ts`, `window-chrome/windows.ts`.

| | Fullscreen | Windowed |
|---|------------|----------|
| Pass-through | Per Settings (`click_through`) | **Off** always |
| Background | Transparent | Opaque `#121216` |
| Disabled setting row | N/A | Grey text + `(fullscreen only)` hint; **no hover tooltip** (VS Code pattern); click → red top toast |

Do not enable `setIgnoreCursorEvents(true)` when `data-window-mode="windowed"` or when poll runs with pass-through disabled.

---

## Split tree vs DOM

The split **tree** is the source of truth for persistence; the **DOM** is the source of truth during drag.

| Event | Action |
|-------|--------|
| Sub-grid drag end (same slot) | `onPersistSplit` → `syncSplitTreeFromDom` |
| Cross-slot drop | `applySplitPanelMoveInDom` then `persistSplitTree` |
| Incremental move fails | Sync DOM → retry incremental → **only then** `onTreeChange` / full render |
| Save profile | `syncSplitTreeFromDom` before `captureSplitProfile` |
| Load profile | `applyProfileToSplit` + `ensureSubGridPanelLayouts` + `renderCanvas` |

**Full re-render** (`renderSplitLayout`): detaches existing `.panel-tile` nodes by `panelId` and reuses them to avoid panel extension remount flicker.

---

## Sub-grid (`#`)

- One `.split-subgrid.desktop-grid` host per enabled leaf.
- `pointer-events: none` on sub-grid host; panels have `pointer-events: auto`.
- Drag wired on panel **header** (`wireSubGridPanelDrag`), not delegated only on host.
- Saved `pixelLock` from old profiles must be normalized to col/row via `normalizeSubGridTile`.

---

## Grid stacking

- Default tiles: `z-index: 2` (`.panel-tile-snap` / `.panel-tile-free`).
- While dragging: `.panel-tile-dragging` → `z-index: 10`.
- After drop: last item in `openPanelIds` / DOM `appendChild` order is on top.

---

## Common failure modes

| Symptom | Likely cause |
|---------|----------------|
| Other panel flickers on first cross-slot drag | Incremental move failed → full `renderCanvas` |
| Panel stuck / invisible in sub-grid after load | Stale `pixelLock` or tree/DOM leaf mismatch |
| Cannot drag within sub-grid after load | Cross-slot hit test winning over local reposition |
| Panel resets when moving a second panel | Tree not synced before persist; stale `getTree` |
| Grid lines wrong after profile load | Sub-grid `--cell-size` on shell root instead of `gridHost` |

---

## Manual test checklist

Run after layout changes (empty slots only unless testing swap):

**Grid mode**

- [ ] Drag panel; it stays above others while moving and after drop
- [ ] Resize handle works; snap on/off if toggled
- [ ] Overlapping panels: dragged one comes to front

**Layout mode — simple slots**

- [ ] Drag panel between empty slots
- [ ] Other panel does **not** flicker on first or repeat drags
- [ ] Sash drag; sash snap if enabled

**Layout mode — sub-grid**

- [ ] Load saved mixed profile (grid slot + simple slot)
- [ ] Drag within `#` sub-grid
- [ ] Drag sub-grid panel to empty simple slot and back
- [ ] Save profile → reload → positions and drag still work

**Profiles**

- [ ] Save to slot; load slot; round-trip without manual fix

---

## Persistence keys

- `localStorage` key: `nulqor-shell-v8` (`types.ts` `STORAGE_KEY`)
- Profiles: `canvasProfiles` (5 slots), `activeProfileId`, `canvasMode`, `split`, `panelLayouts`, `openPanelIds`

Re-save old mixed profiles after major layout fixes so sub-grids store normalized cells not bad `pixelLock`.
