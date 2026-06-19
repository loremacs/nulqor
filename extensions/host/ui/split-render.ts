import {
  collectLeaves,
  cloneSplitNode,
  findLeaf,
  findLeafOwningPanel,
  findSplitContainer,
  mergeLeaf,
  movePanelToLeaf,
  replaceSplitContainer,
  SASH_SNAP_THRESHOLD_PX,
  SASH_THICKNESS_PX,
  sashBoundaryLocalPx,
  setSashBoundary,
  splitLeaf,
  syncContainerRatiosFromDom,
  toggleLeafSubGrid,
  type SplitCanvasState,
  type SplitLeaf,
  type SplitNode,
} from "./split-layout";
import { mountPanel, isPanelResizable } from "./panels";
import type { ShellConfig, TileLayout } from "./types";
import {
  clampTileToDesk,
  pointerToGridCell,
  snapTileFromPointer,
  tileDisplayRect,
  tileLayoutFromPixelRect,
  updateGridGeometry,
  minTileColSpan,
  minTileRowSpan,
  type GridMetrics,
} from "./grid";
import { PANEL_MIN_HEIGHT_PX, PANEL_MIN_WIDTH_PX } from "./types";

export type PanelMoveInfo = {
  panelId: string;
  sourceLeafId: string;
  targetLeafId: string;
  displacedPanelId?: string | null;
};

export type SplitRenderOptions = {
  desktop: HTMLElement;
  shellRoot: HTMLElement;
  shell: ShellConfig;
  split: SplitCanvasState;
  layoutEditing: boolean;
  /** Allow dragging panel headers between split slots (layout mode). */
  allowSlotDrag: boolean;
  panelLayouts: Record<string, TileLayout>;
  onTreeChange: (tree: SplitNode, preset: SplitCanvasState["preset"]) => void;
  /** Tree update without re-mounting panels (sub-grid drag, sash ratios). */
  onPersistSplit?: (tree: SplitNode) => void;
  /** Incremental panel reparenting; return true when DOM was patched. */
  tryPanelMove?: (tree: SplitNode, move: PanelMoveInfo) => boolean;
  onClosePanel: (panelId: string) => void;
  suspendClickThrough: () => () => void;
  /** Live split tree (defaults to `split.tree` captured at render time). */
  getTree?: () => SplitNode;
};

function resolveTree(opts: SplitRenderOptions): SplitNode {
  return opts.getTree?.() ?? opts.split.tree;
}

function resolvePanelSourceLeafId(
  desktop: HTMLElement,
  tree: SplitNode,
  panelId: string,
): string | null {
  const panelEl = desktop.querySelector<HTMLElement>(
    `.panel-tile[data-panel-id="${panelId}"]`,
  );
  const domLeafId = panelEl?.closest<HTMLElement>(".split-slot[data-leaf-id]")
    ?.dataset.leafId;
  // DOM slot is authoritative — tree may lag after profile load / incremental moves.
  if (domLeafId) return domLeafId;
  return findLeafOwningPanel(tree, panelId)?.id ?? null;
}

function isPointInRect(
  clientX: number,
  clientY: number,
  rect: DOMRect,
): boolean {
  return (
    clientX >= rect.left &&
    clientX <= rect.right &&
    clientY >= rect.top &&
    clientY <= rect.bottom
  );
}

function leafIdAtPoint(
  clientX: number,
  clientY: number,
  ignore?: HTMLElement | null,
): string | null {
  const ignoreSlot = ignore?.closest<HTMLElement>(".split-slot[data-leaf-id]");

  // Geometric hit test first — reliable for sub-grids (.split-subgrid uses pointer-events: none).
  let best: { id: string; area: number } | null = null;
  for (const slot of document.querySelectorAll<HTMLElement>(
    ".split-slot[data-leaf-id]",
  )) {
    if (ignoreSlot && slot === ignoreSlot) continue;
    const rect = slot.getBoundingClientRect();
    if (
      clientX >= rect.left &&
      clientX <= rect.right &&
      clientY >= rect.top &&
      clientY <= rect.bottom
    ) {
      const area = rect.width * rect.height;
      if (!best || area < best.area) {
        best = { id: slot.dataset.leafId!, area };
      }
    }
  }
  if (best) return best.id;

  for (const el of document.elementsFromPoint(clientX, clientY)) {
    if (!(el instanceof HTMLElement)) continue;
    if (ignore && (el === ignore || ignore.contains(el))) continue;
    const slot = el.closest<HTMLElement>(".split-slot[data-leaf-id]");
    if (slot?.dataset.leafId) return slot.dataset.leafId;
  }

  return null;
}

function resolveDropTargetLeafId(
  clientX: number,
  clientY: number,
  ignore: HTMLElement | null,
  hoverTargetLeafId: string | null,
  sourceLeafId: string | null,
): string | null {
  let targetId = leafIdAtPoint(clientX, clientY, ignore);
  if ((!targetId || targetId === sourceLeafId) && hoverTargetLeafId) {
    targetId = hoverTargetLeafId;
  }
  if (!targetId || !sourceLeafId || targetId === sourceLeafId) return null;
  return targetId;
}

function clearDropTargets(): void {
  for (const slot of document.querySelectorAll<HTMLElement>(
    ".split-slot.is-drop-target",
  )) {
    slot.classList.remove("is-drop-target");
  }
}

function persistSplitTree(opts: SplitRenderOptions, tree: SplitNode): void {
  if (opts.onPersistSplit) {
    opts.onPersistSplit(tree);
  } else {
    opts.onTreeChange(tree, opts.split.preset);
  }
}

function simpleSlotPanelId(slot: HTMLElement): string | null {
  return (
    slot.querySelector<HTMLElement>(":scope > .panel-tile[data-panel-id]")
      ?.dataset.panelId ?? null
  );
}

function displacedPanelAtDrop(
  desktop: HTMLElement,
  targetLeafId: string,
  targetLeafBefore: SplitLeaf | null,
  draggedPanelId: string,
): string | null {
  if (!targetLeafBefore || targetLeafBefore.subGrid?.enabled) return null;
  const targetSlot = desktop.querySelector<HTMLElement>(
    `.split-slot[data-leaf-id="${targetLeafId}"]`,
  );
  if (!targetSlot) return null;
  const existing = simpleSlotPanelId(targetSlot);
  if (existing && existing !== draggedPanelId) return existing;
  return null;
}

function executePanelDrop(
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
  panelId: string,
  targetLeafId: string,
): void {
  const treeBefore = getTree();
  const sourceLeafId = resolvePanelSourceLeafId(
    opts.desktop,
    treeBefore,
    panelId,
  );
  if (!sourceLeafId || sourceLeafId === targetLeafId) return;

  const targetLeafBefore = findLeaf(treeBefore, targetLeafId);
  const displacedPanelId = displacedPanelAtDrop(
    opts.desktop,
    targetLeafId,
    targetLeafBefore,
    panelId,
  );

  const next = movePanelToLeaf(
    treeBefore,
    panelId,
    targetLeafId,
    opts.panelLayouts,
    sourceLeafId,
  );
  commitPanelMove(opts, next, {
    panelId,
    sourceLeafId,
    targetLeafId,
    displacedPanelId,
  });
}

function applySimpleSlotPanelStyle(
  tileEl: HTMLElement,
  layoutEditing: boolean,
): void {
  tileEl.style.position = layoutEditing ? "relative" : "absolute";
  if (layoutEditing) {
    tileEl.style.flex = "1 1 auto";
    tileEl.style.minHeight = "0";
    tileEl.style.inset = "";
    tileEl.style.left = "";
    tileEl.style.top = "";
    tileEl.style.width = "100%";
    tileEl.style.height = "auto";
  } else {
    tileEl.style.flex = "";
    tileEl.style.minHeight = "";
    tileEl.style.inset = "0";
    tileEl.style.left = "";
    tileEl.style.top = "";
    tileEl.style.width = "100%";
    tileEl.style.height = "100%";
  }
}

function slotSubGrid(slot: HTMLElement): HTMLElement | null {
  return slot.querySelector<HTMLElement>(":scope > .split-subgrid");
}

function syncEmptySlotPlaceholder(
  slot: HTMLElement,
  leaf: SplitLeaf,
  layoutEditing: boolean,
): void {
  slot.querySelector(".split-slot-empty")?.remove();
  if (!layoutEditing) return;
  const hasPanel =
    leaf.panelId !== null ||
    (leaf.subGrid?.enabled && leaf.subGrid.openPanelIds.length > 0);
  if (hasPanel) return;
  const empty = document.createElement("div");
  empty.className = "split-slot-empty";
  empty.textContent = "Empty slot";
  slot.append(empty);
}

/** Reparent panel tiles after movePanelToLeaf — avoids full desktop rebuild. */
export function applySplitPanelMoveInDom(
  desktop: HTMLElement,
  tree: SplitNode,
  move: PanelMoveInfo,
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
): boolean {
  const { panelId, sourceLeafId, targetLeafId, displacedPanelId } = move;
  let sourceLeaf = findLeaf(tree, sourceLeafId);
  const targetLeaf = findLeaf(tree, targetLeafId);
  if (!sourceLeaf || !targetLeaf) return false;

  const targetSlot = desktop.querySelector<HTMLElement>(
    `.split-slot[data-leaf-id="${targetLeafId}"]`,
  );
  if (!targetSlot) return false;

  const panelEl = desktop.querySelector<HTMLElement>(
    `.panel-tile[data-panel-id="${panelId}"]`,
  );
  if (!panelEl) return false;

  const sourceSlot = panelEl.closest<HTMLElement>(".split-slot[data-leaf-id]");
  if (!sourceSlot) return false;
  const effectiveSourceLeafId = sourceSlot.dataset.leafId!;
  if (effectiveSourceLeafId !== sourceLeafId) {
    sourceLeaf = findLeaf(tree, effectiveSourceLeafId);
    if (!sourceLeaf) return false;
  }

  const placeInSimpleSlot = (slot: HTMLElement, tileEl: HTMLElement): void => {
    slot.querySelector(".split-slot-empty")?.remove();
    slot.append(tileEl);
    applySimpleSlotPanelStyle(tileEl, opts.layoutEditing);
    const header = tileEl.querySelector<HTMLElement>(".panel-tile-header");
    if (header) delete header.dataset.slotDragWired;
  };

  const placeInSubGridSlot = (
    slot: HTMLElement,
    leaf: SplitLeaf,
    pid: string,
    tileEl: HTMLElement,
  ): boolean => {
    const gridHost = slotSubGrid(slot);
    if (!gridHost || !leaf.subGrid?.enabled) return false;
    const sub = leaf.subGrid;
    const subShell: ShellConfig = {
      ...opts.shell,
      cell_pixels: sub.cell_pixels,
      snap_enabled: sub.snap_enabled,
      show_grid: sub.show_grid,
    };
    const metrics = updateGridGeometry(opts.shellRoot, gridHost, subShell, gridHost);
    let layout = sub.panelLayouts[pid];
    if (!layout) return false;
    layout = clampTileToDesk(
      { ...layout, pixelLock: undefined, freeX: undefined, freeY: undefined },
      metrics,
    );
    sub.panelLayouts[pid] = layout;
    const header = tileEl.querySelector<HTMLElement>(".panel-tile-header");
    if (header) {
      delete header.dataset.slotDragWired;
      delete header.dataset.subGridDragWired;
    }
    gridHost.append(tileEl);
    applyTileStyle(tileEl, tileDisplayRect(layout, metrics, sub.snap_enabled));
    wireSubGridPanelDrag(tileEl, pid, leaf.id, gridHost, opts, getTree, () => {
      const live = findLeaf(getTree(), leaf.id)?.subGrid;
      const subShell: ShellConfig = {
        ...opts.shell,
        cell_pixels: live?.cell_pixels ?? sub.cell_pixels,
        snap_enabled: live?.snap_enabled ?? sub.snap_enabled,
        show_grid: live?.show_grid ?? sub.show_grid,
      };
        return updateGridGeometry(opts.shellRoot, gridHost, subShell, gridHost);
    });
    return true;
  };

  if (targetLeaf.subGrid?.enabled) {
    if (!placeInSubGridSlot(targetSlot, targetLeaf, panelId, panelEl))
      return false;
    panelEl.dataset.sourceLeafId = targetLeafId;
  } else {
    placeInSimpleSlot(targetSlot, panelEl);
    panelEl.dataset.sourceLeafId = targetLeafId;
    if (opts.allowSlotDrag) {
      delete panelEl.querySelector<HTMLElement>(".panel-tile-header")?.dataset
        .slotDragWired;
      wireSlotPanelDrag(panelEl, panelId, targetLeafId, opts, getTree);
    }
  }

  if (displacedPanelId) {
    const displacedEl = desktop.querySelector<HTMLElement>(
      `.panel-tile[data-panel-id="${displacedPanelId}"]`,
    );
    if (!displacedEl) return false;

    if (
      sourceLeaf.subGrid?.enabled &&
      sourceLeaf.subGrid.openPanelIds.includes(displacedPanelId)
    ) {
      if (
        !placeInSubGridSlot(
          sourceSlot,
          sourceLeaf,
          displacedPanelId,
          displacedEl,
        )
      ) {
        return false;
      }
      displacedEl.dataset.sourceLeafId = sourceLeafId;
    } else if (sourceLeaf.panelId === displacedPanelId) {
      placeInSimpleSlot(sourceSlot, displacedEl);
      displacedEl.dataset.sourceLeafId = sourceLeafId;
      if (opts.allowSlotDrag) {
        wireSlotPanelDrag(
          displacedEl,
          displacedPanelId,
          sourceLeafId,
          opts,
          getTree,
        );
      }
    } else {
      return false;
    }
  }

  syncEmptySlotPlaceholder(sourceSlot, sourceLeaf, opts.layoutEditing);
  syncEmptySlotPlaceholder(targetSlot, targetLeaf, opts.layoutEditing);
  return true;
}

function commitPanelMove(
  opts: SplitRenderOptions,
  tree: SplitNode,
  move: PanelMoveInfo,
): void {
  if (opts.tryPanelMove?.(tree, move)) {
    persistSplitTree(opts, tree);
    return;
  }
  // Incremental patch failed — sync DOM, retry once, full rebuild only as last resort.
  const liveTree = resolveTree(opts);
  const domTree = syncSplitTreeFromDom(opts.desktop, liveTree);
  const merged = movePanelToLeaf(
    domTree,
    move.panelId,
    move.targetLeafId,
    opts.panelLayouts,
    move.sourceLeafId,
  );
  if (opts.tryPanelMove?.(merged, move)) {
    persistSplitTree(opts, merged);
    return;
  }
  opts.onTreeChange(merged, "custom");
}

function wireSlotPanelDrag(
  tileEl: HTMLElement,
  panelId: string,
  sourceLeafId: string,
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
): void {
  if (!opts.allowSlotDrag) return;
  const header = tileEl.querySelector<HTMLElement>(".panel-tile-header");
  if (!header) return;

  tileEl.dataset.sourceLeafId = sourceLeafId;
  header.style.cursor = "grab";

  if (header.dataset.slotDragWired === "1") return;
  header.dataset.slotDragWired = "1";

  header.addEventListener("pointerdown", (event) => {
    if ((event.target as HTMLElement).closest("button")) return;
    event.preventDefault();
    header.setPointerCapture(event.pointerId);
    const resume = opts.suspendClickThrough();
    tileEl.classList.add("panel-tile-dragging");
    let hoverTargetLeafId: string | null = null;

    const move = (e: PointerEvent): void => {
      clearDropTargets();
      const sourceLeafId = resolvePanelSourceLeafId(
        opts.desktop,
        getTree(),
        panelId,
      );
      hoverTargetLeafId = resolveDropTargetLeafId(
        e.clientX,
        e.clientY,
        tileEl,
        null,
        sourceLeafId,
      );
      if (hoverTargetLeafId) {
        document
          .querySelector<HTMLElement>(
            `.split-slot[data-leaf-id="${hoverTargetLeafId}"]`,
          )
          ?.classList.add("is-drop-target");
      }
    };

    const end = (e: PointerEvent): void => {
      header.releasePointerCapture(e.pointerId);
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", end);
      document.removeEventListener("pointercancel", end);
      tileEl.classList.remove("panel-tile-dragging");
      clearDropTargets();
      resume();

      const sourceLeafId = resolvePanelSourceLeafId(
        opts.desktop,
        getTree(),
        panelId,
      );
      const targetId = resolveDropTargetLeafId(
        e.clientX,
        e.clientY,
        tileEl,
        hoverTargetLeafId,
        sourceLeafId,
      );
      if (targetId) executePanelDrop(opts, getTree, panelId, targetId);
    };

    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", end);
    document.addEventListener("pointercancel", end);
  });
}

function defaultSubGridTile(
  id: string,
  index: number,
  metrics: GridMetrics,
): TileLayout {
  const colSpan = Math.min(4, metrics.cols);
  const rowSpan = Math.min(3, metrics.rows);
  const col = 1 + (index % Math.max(1, metrics.cols - colSpan + 1));
  const row =
    1 + Math.floor(index / Math.max(1, metrics.cols - colSpan + 1)) * rowSpan;
  return { id, col, row, colSpan, rowSpan };
}

/** Convert saved pixelLock / legacy layouts into clamped grid cells for this sub-grid host. */
function normalizeSubGridTile(
  tile: TileLayout,
  metrics: GridMetrics,
): TileLayout {
  const base = tile.pixelLock
    ? {
        ...tileLayoutFromPixelRect(tile.pixelLock, metrics, tile.id),
        pixelLock: undefined,
        freeX: undefined,
        freeY: undefined,
      }
    : tile;
  return clampTileToDesk(base, metrics);
}

function subGridForLeaf(
  getTree: () => SplitNode,
  leafId: string,
): SplitLeaf["subGrid"] | null {
  const leaf = findLeaf(getTree(), leafId);
  return leaf?.subGrid?.enabled ? leaf.subGrid : null;
}

function applyTileStyle(
  el: HTMLElement,
  rect: { left: number; top: number; width: number; height: number },
): void {
  el.style.flex = "";
  el.style.minHeight = "";
  el.style.inset = "";
  el.style.position = "absolute";
  el.style.left = `${rect.left}px`;
  el.style.top = `${rect.top}px`;
  el.style.width = `${Math.max(rect.width, PANEL_MIN_WIDTH_PX)}px`;
  el.style.height = `${Math.max(rect.height, PANEL_MIN_HEIGHT_PX)}px`;
}

async function mountPanelTile(
  panelId: string,
  host: HTMLElement,
  opts: {
    draggable: boolean;
    onClose: () => void;
  },
  preserved?: Map<string, HTMLElement>,
): Promise<HTMLElement> {
  const existing = preserved?.get(panelId);
  if (existing) {
    preserved!.delete(panelId);
    const header = existing.querySelector<HTMLElement>(".panel-tile-header");
    if (header) {
      delete header.dataset.slotDragWired;
      delete header.dataset.subGridDragWired;
    }
    host.append(existing);
    return existing;
  }

  const el = document.createElement("article");
  el.className = "panel-tile panel-tile-split";
  el.dataset.panelId = panelId;
  el.dataset.interactive = "true";

  const header = document.createElement("header");
  header.className = "panel-tile-header";
  header.dataset.interactive = "true";
  header.innerHTML = `<span>${panelId}</span><button type="button" data-close="${panelId}" title="Close">×</button>`;
  header.querySelector("button")!.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onClose();
  });

  const body = document.createElement("div");
  body.className = "panel-tile-body";
  body.dataset.interactive = "true";

  el.append(header, body);
  if (opts.draggable && isPanelResizable(panelId)) {
    const handle = document.createElement("div");
    handle.className = "panel-resize-handle";
    handle.dataset.interactive = "true";
    el.append(handle);
  }

  host.append(el);
  await mountPanel(panelId, body);
  return el;
}

function detachPanelTiles(desktop: HTMLElement): Map<string, HTMLElement> {
  const map = new Map<string, HTMLElement>();
  for (const el of desktop.querySelectorAll<HTMLElement>(
    ".panel-tile[data-panel-id]",
  )) {
    const id = el.dataset.panelId;
    if (!id || map.has(id)) continue;
    el.remove();
    map.set(id, el);
  }
  return map;
}

async function renderSubGrid(
  leafEl: HTMLElement,
  leafNode: SplitLeaf,
  shell: ShellConfig,
  shellRoot: HTMLElement,
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
  preserved?: Map<string, HTMLElement>,
): Promise<void> {
  const leafId = leafNode.id;
  const gridHost = document.createElement("div");
  gridHost.className = "split-subgrid desktop-grid";
  if (leafNode.subGrid?.show_grid) gridHost.classList.add("show-subgrid-lines");
  leafEl.append(gridHost);

  const subGridShell = (): ShellConfig => {
    const sub = subGridForLeaf(getTree, leafId);
    return {
      ...shell,
      cell_pixels: sub?.cell_pixels ?? shell.cell_pixels,
      snap_enabled: sub?.snap_enabled ?? shell.snap_enabled,
      show_grid: sub?.show_grid ?? shell.show_grid,
    };
  };

  let metrics = updateGridGeometry(shellRoot, gridHost, subGridShell(), gridHost);

  const panelIds = () =>
    subGridForLeaf(getTree, leafId)?.openPanelIds.filter(Boolean) ?? [];

  for (let i = 0; i < panelIds().length; i++) {
    const pid = panelIds()[i];
    const sub = subGridForLeaf(getTree, leafId);
    if (!sub) continue;
    const tile = normalizeSubGridTile(
      sub.panelLayouts[pid] ?? defaultSubGridTile(pid, i, metrics),
      metrics,
    );
    sub.panelLayouts[pid] = tile;
    const el = await mountPanelTile(
      pid,
      gridHost,
      {
        draggable: true,
        onClose: () => opts.onClosePanel(pid),
      },
      preserved,
    );
    applyTileStyle(el, tileDisplayRect(tile, metrics, sub.snap_enabled));
    wireSubGridPanelDrag(el, pid, leafId, gridHost, opts, getTree, () => {
      metrics = updateGridGeometry(shellRoot, gridHost, subGridShell(), gridHost);
      return metrics;
    });
  }
}

function wireSubGridPanelDrag(
  tileEl: HTMLElement,
  panelId: string,
  leafId: string,
  gridHost: HTMLElement,
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
  refreshMetrics: () => GridMetrics,
): void {
  const header = tileEl.querySelector<HTMLElement>(".panel-tile-header");
  if (!header || header.dataset.subGridDragWired === "1") return;
  header.dataset.subGridDragWired = "1";

  const handle = tileEl.querySelector<HTMLElement>(".panel-resize-handle");
  handle?.addEventListener("pointerdown", (event) => {
    const live = subGridForLeaf(getTree, leafId);
    const id = panelId;
    if (!live?.panelLayouts[id]) return;
    event.preventDefault();
    event.stopPropagation();
    const metrics = refreshMetrics();
    const resume = opts.suspendClickThrough();
    const move = (e: PointerEvent): void => {
      const sub = subGridForLeaf(getTree, leafId);
      const tile = sub?.panelLayouts[id];
      if (!tile || !sub) return;
      const endCell = pointerToGridCell(
        e.clientX,
        e.clientY,
        gridHost,
        metrics,
      );
      const colSpan = Math.max(
        minTileColSpan(metrics),
        endCell.col - tile.col + 1,
      );
      const rowSpan = Math.max(
        minTileRowSpan(metrics),
        endCell.row - tile.row + 1,
      );
      const next = clampTileToDesk(
        {
          ...tile,
          colSpan,
          rowSpan,
          pixelLock: undefined,
          freeX: undefined,
          freeY: undefined,
        },
        metrics,
      );
      sub.panelLayouts[id] = next;
      applyTileStyle(tileEl, tileDisplayRect(next, metrics, sub.snap_enabled));
    };
    const end = (): void => {
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", end);
      document.removeEventListener("pointercancel", end);
      resume();
      persistSplitTree(opts, getTree());
    };
    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", end);
    document.addEventListener("pointercancel", end);
  });

  header.addEventListener("pointerdown", (event) => {
    if ((event.target as HTMLElement).closest("button")) return;
    const live = subGridForLeaf(getTree, leafId);
    if (!live?.panelLayouts[panelId]) return;

    const metrics = refreshMetrics();
    live.panelLayouts[panelId] = normalizeSubGridTile(
      live.panelLayouts[panelId],
      metrics,
    );
    const rect = tileEl.getBoundingClientRect();
    const ox = event.clientX - rect.left;
    const oy = event.clientY - rect.top;
    event.preventDefault();
    event.stopPropagation();
    header.setPointerCapture(event.pointerId);
    const resume = opts.suspendClickThrough();
    tileEl.classList.add("panel-tile-dragging");
    let hoverTargetLeafId: string | null = null;

    const repositionInSubGrid = (e: PointerEvent): void => {
      const sub = subGridForLeaf(getTree, leafId);
      const tile = sub?.panelLayouts[panelId];
      if (!tile || !sub) return;
      const gridRect = gridHost.getBoundingClientRect();
      const topLeftX = e.clientX - ox;
      const topLeftY = e.clientY - oy;
      let next: TileLayout;
      if (!sub.snap_enabled) {
        next = {
          ...tile,
          freeX: e.clientX - ox - gridRect.left,
          freeY: e.clientY - oy - gridRect.top,
          pixelLock: undefined,
        };
      } else {
        next = snapTileFromPointer(tile, topLeftX, topLeftY, gridHost, metrics);
      }
      sub.panelLayouts[panelId] = next;
      applyTileStyle(tileEl, tileDisplayRect(next, metrics, sub.snap_enabled));
    };

    const move = (e: PointerEvent): void => {
      const sub = subGridForLeaf(getTree, leafId);
      if (!sub?.panelLayouts[panelId]) return;
      clearDropTargets();
      const gridRect = gridHost.getBoundingClientRect();

      // While the pointer stays inside this sub-grid, always reposition locally.
      if (isPointInRect(e.clientX, e.clientY, gridRect)) {
        hoverTargetLeafId = null;
        repositionInSubGrid(e);
        return;
      }

      const sourceLeafId =
        resolvePanelSourceLeafId(opts.desktop, getTree(), panelId) ?? leafId;
      hoverTargetLeafId = resolveDropTargetLeafId(
        e.clientX,
        e.clientY,
        tileEl,
        hoverTargetLeafId,
        sourceLeafId,
      );
      if (hoverTargetLeafId) {
        document
          .querySelector<HTMLElement>(
            `.split-slot[data-leaf-id="${hoverTargetLeafId}"]`,
          )
          ?.classList.add("is-drop-target");
      }
    };

    const end = (e: PointerEvent): void => {
      header.releasePointerCapture(e.pointerId);
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", end);
      document.removeEventListener("pointercancel", end);
      tileEl.classList.remove("panel-tile-dragging");
      resume();
      clearDropTargets();

      const gridRect = gridHost.getBoundingClientRect();
      if (isPointInRect(e.clientX, e.clientY, gridRect)) {
        repositionInSubGrid(e);
        persistSplitTree(opts, getTree());
        return;
      }

      const sourceLeafId =
        resolvePanelSourceLeafId(opts.desktop, getTree(), panelId) ?? leafId;
      const targetLeaf = resolveDropTargetLeafId(
        e.clientX,
        e.clientY,
        tileEl,
        hoverTargetLeafId,
        sourceLeafId,
      );
      if (targetLeaf) {
        executePanelDrop(opts, getTree, panelId, targetLeaf);
        return;
      }
      persistSplitTree(opts, getTree());
    };

    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", end);
    document.addEventListener("pointercancel", end);
  });
}

function applyFlexFromContainer(
  containerEl: HTMLElement,
  container: import("./split-layout").SplitContainer,
): void {
  const panes = Array.from(
    containerEl.querySelectorAll<HTMLElement>(":scope > .split-pane"),
  );
  panes.forEach((pane, index) => {
    const ratio = container.children[index]?.ratio;
    if (ratio !== undefined) pane.style.flex = `${ratio} 1 0`;
  });
}

function collectSashSnapTargets(
  desktop: HTMLElement,
  sashClass: "split-sash-vertical" | "split-sash-horizontal",
  excludeSash: HTMLElement,
): number[] {
  const targets: number[] = [];
  for (const el of desktop.querySelectorAll<HTMLElement>(`.${sashClass}`)) {
    if (el === excludeSash) continue;
    const rect = el.getBoundingClientRect();
    targets.push(
      sashClass === "split-sash-vertical"
        ? rect.left + rect.width / 2
        : rect.top + rect.height / 2,
    );
  }
  return targets;
}

function findSnapTarget(
  value: number,
  targets: number[],
  threshold: number,
): number | null {
  let best: number | null = null;
  let bestDist = threshold + 1;
  for (const target of targets) {
    const dist = Math.abs(target - value);
    if (dist <= threshold && dist < bestDist) {
      bestDist = dist;
      best = target;
    }
  }
  return best;
}

function snapToNearest(
  value: number,
  targets: number[],
  threshold: number,
): number {
  return findSnapTarget(value, targets, threshold) ?? value;
}

function getSashSnapPreview(): HTMLElement {
  let el = document.querySelector<HTMLElement>(".sash-snap-preview");
  if (!el) {
    el = document.createElement("div");
    el.className = "sash-snap-preview";
    el.setAttribute("aria-hidden", "true");
    el.hidden = true;
    document.querySelector(".desktop-canvas")?.appendChild(el);
  }
  return el;
}

function showSashSnapPreview(
  _shellRoot: HTMLElement,
  desktop: HTMLElement,
  orientation: "vertical" | "horizontal",
  centerClient: number,
): void {
  const el = getSashSnapPreview();
  const desk = desktop.getBoundingClientRect();
  const thickness = 3;
  const half = thickness / 2;
  el.dataset.orientation = orientation;
  if (orientation === "vertical") {
    el.style.left = `${centerClient - half}px`;
    el.style.top = `${desk.top}px`;
    el.style.width = `${thickness}px`;
    el.style.height = `${desk.height}px`;
  } else {
    el.style.left = `${desk.left}px`;
    el.style.top = `${centerClient - half}px`;
    el.style.width = `${desk.width}px`;
    el.style.height = `${thickness}px`;
  }
  el.hidden = false;
  requestAnimationFrame(() => {
    el.classList.add("is-visible");
  });
}

function hideSashSnapPreview(): void {
  const el = document.querySelector<HTMLElement>(".sash-snap-preview");
  if (!el) return;
  el.classList.remove("is-visible");
  el.hidden = true;
}

function updateSashSnapPreviewDuringDrag(
  opts: SplitRenderOptions,
  sash: HTMLElement,
  isHorizontal: boolean,
  containerRect: DOMRect,
  boundaryLocal: number,
): void {
  if (!opts.shell.sash_snap_enabled) {
    hideSashSnapPreview();
    return;
  }
  const sashCenterClient = isHorizontal
    ? containerRect.left + boundaryLocal + SASH_THICKNESS_PX / 2
    : containerRect.top + boundaryLocal + SASH_THICKNESS_PX / 2;
  const sashClass = isHorizontal
    ? "split-sash-vertical"
    : "split-sash-horizontal";
  const targets = collectSashSnapTargets(opts.desktop, sashClass, sash);
  const snapTarget = findSnapTarget(
    sashCenterClient,
    targets,
    SASH_SNAP_THRESHOLD_PX,
  );
  if (snapTarget === null) {
    hideSashSnapPreview();
    return;
  }
  showSashSnapPreview(
    opts.shellRoot,
    opts.desktop,
    isHorizontal ? "vertical" : "horizontal",
    snapTarget,
  );
}

function wireSash(
  sash: HTMLElement,
  containerEl: HTMLElement,
  containerId: string,
  sashIndex: number,
  getTree: () => SplitNode,
  opts: SplitRenderOptions,
): void {
  sash.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    sash.setPointerCapture(event.pointerId);
    const resume = opts.suspendClickThrough();
    const isHorizontal = containerEl.classList.contains("split-horizontal");
    let pendingTree: SplitNode | null = null;
    let boundaryLocal = 0;

    const containerRect = (): DOMRect => containerEl.getBoundingClientRect();
    const containerSize = (): number => {
      const rect = containerRect();
      return isHorizontal ? rect.width : rect.height;
    };
    const pointerLocal = (e: PointerEvent): number => {
      const rect = containerRect();
      return isHorizontal ? e.clientX - rect.left : e.clientY - rect.top;
    };

    let tree = getTree();
    let container = findSplitContainer(tree, containerId);
    if (container) {
      const synced = syncContainerRatiosFromDom(containerEl, container);
      pendingTree = replaceSplitContainer(tree, containerId, synced);
      applyFlexFromContainer(containerEl, synced);
      container = synced;
    }

    boundaryLocal = sashBoundaryLocalPx(containerEl, sashIndex, isHorizontal);
    const grabOffset = pointerLocal(event) - boundaryLocal;

    const applyBoundary = (boundary: number): void => {
      tree = pendingTree ?? getTree();
      container = findSplitContainer(tree, containerId);
      if (!container) return;
      const updated = setSashBoundary(
        container,
        sashIndex,
        boundary,
        containerSize(),
      );
      pendingTree = replaceSplitContainer(tree, containerId, updated);
      applyFlexFromContainer(containerEl, updated);
      boundaryLocal = boundary;
    };

    const move = (e: PointerEvent): void => {
      e.preventDefault();
      applyBoundary(pointerLocal(e) - grabOffset);
      updateSashSnapPreviewDuringDrag(
        opts,
        sash,
        isHorizontal,
        containerRect(),
        boundaryLocal,
      );
    };

    const end = (e: PointerEvent): void => {
      if (sash.hasPointerCapture(e.pointerId)) {
        sash.releasePointerCapture(e.pointerId);
      }
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", end);
      document.removeEventListener("pointercancel", end);
      hideSashSnapPreview();

      if (opts.shell.sash_snap_enabled && pendingTree) {
        const rect = containerRect();
        const sashCenterClient = isHorizontal
          ? rect.left + boundaryLocal + SASH_THICKNESS_PX / 2
          : rect.top + boundaryLocal + SASH_THICKNESS_PX / 2;
        const sashClass = isHorizontal
          ? "split-sash-vertical"
          : "split-sash-horizontal";
        const targets = collectSashSnapTargets(opts.desktop, sashClass, sash);
        const snappedCenter = snapToNearest(
          sashCenterClient,
          targets,
          SASH_SNAP_THRESHOLD_PX,
        );
        if (Math.abs(snappedCenter - sashCenterClient) > 0.5) {
          const snappedBoundary =
            snappedCenter -
            (isHorizontal ? rect.left : rect.top) -
            SASH_THICKNESS_PX / 2;
          applyBoundary(snappedBoundary);
        }
      }

      if (pendingTree) persistSplitTree(opts, pendingTree);
      resume();
    };

    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", end);
    document.addEventListener("pointercancel", end);
  });
}

function renderEditChrome(
  slot: HTMLElement,
  leafId: string,
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
): void {
  const bar = document.createElement("div");
  bar.className = "split-slot-edit-bar";
  bar.dataset.interactive = "true";
  bar.innerHTML = `
    <button type="button" data-split-h title="Split columns">+↔</button>
    <button type="button" data-split-v title="Split rows">+↕</button>
    <button type="button" data-merge title="Merge">−</button>
    <button type="button" data-subgrid title="Toggle sub-grid">#</button>
  `;

  bar.querySelector("[data-split-h]")!.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onTreeChange(splitLeaf(getTree(), leafId, "horizontal"), "custom");
  });
  bar.querySelector("[data-split-v]")!.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onTreeChange(splitLeaf(getTree(), leafId, "vertical"), "custom");
  });
  bar.querySelector("[data-merge]")!.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onTreeChange(mergeLeaf(getTree(), leafId), "custom");
  });
  bar.querySelector("[data-subgrid]")!.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onTreeChange(
      toggleLeafSubGrid(getTree(), leafId, opts.shell),
      "custom",
    );
  });

  slot.append(bar);
}

async function renderNode(
  node: SplitNode,
  host: HTMLElement,
  opts: SplitRenderOptions,
  getTree: () => SplitNode,
  preserved?: Map<string, HTMLElement>,
): Promise<void> {
  if (node.type === "leaf") {
    const slot = document.createElement("div");
    slot.className = "split-slot";
    slot.dataset.leafId = node.id;
    slot.dataset.interactive = "true";
    slot.style.flex = "1 1 0";
    host.append(slot);

    if (opts.layoutEditing) {
      renderEditChrome(slot, node.id, opts, getTree);
    }

    if (node.subGrid?.enabled) {
      await renderSubGrid(
        slot,
        node,
        opts.shell,
        opts.shellRoot,
        opts,
        getTree,
        preserved,
      );
      return;
    }

    if (node.panelId) {
      const tile = await mountPanelTile(
        node.panelId,
        slot,
        {
          draggable: opts.allowSlotDrag,
          onClose: () => opts.onClosePanel(node.panelId!),
        },
        preserved,
      );
      wireSlotPanelDrag(tile, node.panelId, node.id, opts, getTree);
      applySimpleSlotPanelStyle(tile, opts.layoutEditing);
    } else if (opts.layoutEditing) {
      const empty = document.createElement("div");
      empty.className = "split-slot-empty";
      empty.textContent = "Empty slot";
      slot.append(empty);
    }
    return;
  }

  const container = document.createElement("div");
  container.className = `split-container split-${node.direction === "horizontal" ? "horizontal" : "vertical"}`;
  container.dataset.splitId = node.id;
  container.dataset.interactive = "true";
  container.style.flex = "1 1 0";
  container.style.display = "flex";
  container.style.flexDirection =
    node.direction === "horizontal" ? "row" : "column";
  container.style.minWidth = "0";
  container.style.minHeight = "0";
  host.append(container);

  for (let index = 0; index < node.children.length; index++) {
    const child = node.children[index];
    const pane = document.createElement("div");
    pane.className = "split-pane";
    pane.style.flex = `${child.ratio} 1 0`;
    pane.style.minWidth = `${PANEL_MIN_WIDTH_PX}px`;
    pane.style.minHeight = `${PANEL_MIN_HEIGHT_PX}px`;
    pane.style.display = "flex";
    container.append(pane);
    await renderNode(child.node, pane, opts, getTree, preserved);

    if (index < node.children.length - 1) {
      const sash = document.createElement("div");
      sash.className = `split-sash split-sash-${node.direction === "horizontal" ? "vertical" : "horizontal"}`;
      sash.dataset.interactive = "true";
      container.append(sash);
      wireSash(sash, container, node.id, index, getTree, opts);
    }
  }
}

export async function renderSplitLayout(
  opts: SplitRenderOptions,
): Promise<void> {
  const preservedPanels = detachPanelTiles(opts.desktop);
  opts.desktop.innerHTML = "";
  opts.desktop.className = "desktop-canvas desktop-split";
  if (opts.layoutEditing) opts.desktop.classList.add("canvas-editing");

  const getTree = (): SplitNode => resolveTree(opts);
  const renderOpts: SplitRenderOptions = {
    ...opts,
    tryPanelMove: (tree, move) =>
      applySplitPanelMoveInDom(opts.desktop, tree, move, opts, getTree),
  };

  const root = document.createElement("div");
  root.className = "split-root";
  root.dataset.interactive = "true";
  opts.desktop.append(root);

  await renderNode(resolveTree(opts), root, renderOpts, getTree, preservedPanels);
}

function tileLayoutFromSubGridDom(
  tileEl: HTMLElement,
  gridHost: HTMLElement,
  pid: string,
  existing: TileLayout | undefined,
): TileLayout {
  const rect = tileEl.getBoundingClientRect();
  const gridRect = gridHost.getBoundingClientRect();
  const base = existing ?? { id: pid, col: 1, row: 1, colSpan: 4, rowSpan: 3 };
  return {
    ...base,
    id: pid,
    pixelLock: {
      left: Math.max(0, rect.left - gridRect.left),
      top: Math.max(0, rect.top - gridRect.top),
      width: rect.width,
      height: rect.height,
    },
    freeX: undefined,
    freeY: undefined,
  };
}

/** Flush live DOM panel slots, sub-grid positions, and sash ratios into the tree. */
export function syncSplitTreeFromDom(
  desktop: HTMLElement,
  tree: SplitNode,
): SplitNode {
  let next = cloneSplitNode(tree);

  for (const leaf of collectLeaves(next)) {
    const slot = desktop.querySelector<HTMLElement>(
      `.split-slot[data-leaf-id="${leaf.id}"]`,
    );
    if (!slot) continue;

    if (leaf.subGrid?.enabled) {
      const gridHost = slot.querySelector<HTMLElement>(
        ":scope > .split-subgrid",
      );
      if (!gridHost) continue;
      const sub = leaf.subGrid;
      const ids: string[] = [];
      for (const tileEl of gridHost.querySelectorAll<HTMLElement>(
        ":scope > .panel-tile[data-panel-id]",
      )) {
        const pid = tileEl.dataset.panelId;
        if (!pid || ids.includes(pid)) continue;
        ids.push(pid);
        sub.panelLayouts[pid] = tileLayoutFromSubGridDom(
          tileEl,
          gridHost,
          pid,
          sub.panelLayouts[pid],
        );
      }
      sub.openPanelIds = ids;
      leaf.panelId = null;
    } else {
      leaf.panelId = simpleSlotPanelId(slot);
    }
  }

  for (const containerEl of desktop.querySelectorAll<HTMLElement>(
    ".split-container[data-split-id]",
  )) {
    const containerId = containerEl.dataset.splitId;
    if (!containerId) continue;
    const container = findSplitContainer(next, containerId);
    if (!container) continue;
    next = replaceSplitContainer(
      next,
      containerId,
      syncContainerRatiosFromDom(containerEl, container),
    );
  }

  return next;
}

export function removePanelFromTree(
  tree: SplitNode,
  panelId: string,
): SplitNode {
  const cloned = JSON.parse(JSON.stringify(tree)) as SplitNode;
  for (const leafNode of collectLeaves(cloned)) {
    if (leafNode.panelId === panelId) leafNode.panelId = null;
    if (leafNode.subGrid?.enabled) {
      leafNode.subGrid.openPanelIds = leafNode.subGrid.openPanelIds.filter(
        (id) => id !== panelId,
      );
      delete leafNode.subGrid.panelLayouts[panelId];
    }
  }
  return cloned;
}
