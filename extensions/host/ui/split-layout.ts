import type { ShellConfig, TileLayout } from "./types";
import { lockTilePixels, metricsForCellSize } from "./grid";
import { PANEL_MIN_HEIGHT_PX, PANEL_MIN_WIDTH_PX } from "./types";

export type SplitDirection = "horizontal" | "vertical";

export type SubGridState = {
  enabled: boolean;
  cell_pixels: number;
  snap_enabled: boolean;
  show_grid: boolean;
  panelLayouts: Record<string, TileLayout>;
  openPanelIds: string[];
};

export type SplitLeaf = {
  type: "leaf";
  id: string;
  panelId: string | null;
  subGrid?: SubGridState;
};

export type SplitChild = {
  node: SplitNode;
  ratio: number;
};

export type SplitContainer = {
  type: "split";
  id: string;
  direction: SplitDirection;
  children: SplitChild[];
};

export type SplitNode = SplitLeaf | SplitContainer;

export type BuiltInPreset =
  | "single"
  | "two-columns"
  | "three-columns"
  | "two-rows"
  | "three-rows"
  | "grid-2x2"
  | "main-side";

export type SplitShellSnapshot = {
  cell_pixels: number;
  cell_step: number;
  snap_enabled: boolean;
  show_grid: boolean;
  sash_snap_enabled: boolean;
};

export type SplitCanvasState = {
  preset: BuiltInPreset | "custom";
  tree: SplitNode;
  /** Grid/snap settings used when the split layout was saved. */
  shell?: SplitShellSnapshot;
};

/** Flex sash width in CSS (`.split-sash { flex: 0 0 6px }`). */
export const SASH_THICKNESS_PX = 6;
export const SASH_SNAP_THRESHOLD_PX = 12;

export const BUILT_IN_PRESETS: { id: BuiltInPreset; label: string }[] = [
  { id: "single", label: "Single" },
  { id: "two-columns", label: "Two Columns" },
  { id: "three-columns", label: "Three Columns" },
  { id: "two-rows", label: "Two Rows" },
  { id: "three-rows", label: "Three Rows" },
  { id: "grid-2x2", label: "Grid (2×2)" },
  { id: "main-side", label: "Main + Side" },
];

function leaf(id: string): SplitLeaf {
  return { type: "leaf", id, panelId: null };
}

function normalizeChildren(children: SplitChild[]): SplitChild[] {
  const sum = children.reduce((s, c) => s + c.ratio, 0);
  if (sum <= 0) {
    const even = 1 / children.length;
    return children.map((c) => ({ ...c, ratio: even }));
  }
  return children.map((c) => ({ ...c, ratio: c.ratio / sum }));
}

function split(
  id: string,
  direction: SplitDirection,
  children: SplitChild[],
): SplitContainer {
  return {
    type: "split",
    id,
    direction,
    children: normalizeChildren(children),
  };
}

export function buildPreset(preset: BuiltInPreset): SplitNode {
  switch (preset) {
    case "single":
      return leaf("leaf-0");
    case "two-columns":
      return split("root", "horizontal", [
        { node: leaf("leaf-0"), ratio: 0.5 },
        { node: leaf("leaf-1"), ratio: 0.5 },
      ]);
    case "three-columns":
      return split("root", "horizontal", [
        { node: leaf("leaf-0"), ratio: 1 / 3 },
        { node: leaf("leaf-1"), ratio: 1 / 3 },
        { node: leaf("leaf-2"), ratio: 1 / 3 },
      ]);
    case "two-rows":
      return split("root", "vertical", [
        { node: leaf("leaf-0"), ratio: 0.5 },
        { node: leaf("leaf-1"), ratio: 0.5 },
      ]);
    case "three-rows":
      return split("root", "vertical", [
        { node: leaf("leaf-0"), ratio: 1 / 3 },
        { node: leaf("leaf-1"), ratio: 1 / 3 },
        { node: leaf("leaf-2"), ratio: 1 / 3 },
      ]);
    case "grid-2x2":
      return split("root", "vertical", [
        {
          node: split("row-top", "horizontal", [
            { node: leaf("leaf-0"), ratio: 0.5 },
            { node: leaf("leaf-1"), ratio: 0.5 },
          ]),
          ratio: 0.5,
        },
        {
          node: split("row-bottom", "horizontal", [
            { node: leaf("leaf-2"), ratio: 0.5 },
            { node: leaf("leaf-3"), ratio: 0.5 },
          ]),
          ratio: 0.5,
        },
      ]);
    case "main-side":
      return split("root", "horizontal", [
        { node: leaf("leaf-0"), ratio: 0.6 },
        {
          node: split("side", "vertical", [
            { node: leaf("leaf-1"), ratio: 0.5 },
            { node: leaf("leaf-2"), ratio: 0.5 },
          ]),
          ratio: 0.4,
        },
      ]);
  }
}

export function cloneSplitNode(node: SplitNode): SplitNode {
  if (node.type === "leaf") {
    return {
      ...node,
      subGrid: node.subGrid
        ? {
            ...node.subGrid,
            openPanelIds: [...node.subGrid.openPanelIds],
            panelLayouts: Object.fromEntries(
              Object.entries(node.subGrid.panelLayouts).map(([id, tile]) => [
                id,
                { ...tile },
              ]),
            ),
          }
        : undefined,
    };
  }
  return {
    ...node,
    children: node.children.map((c) => ({
      node: cloneSplitNode(c.node),
      ratio: c.ratio,
    })),
  };
}

export function collectLeaves(node: SplitNode): SplitLeaf[] {
  if (node.type === "leaf") return [node];
  const out: SplitLeaf[] = [];
  for (const child of node.children) {
    out.push(...collectLeaves(child.node));
  }
  return out;
}

export function assignPanelsToTree(
  tree: SplitNode,
  panelIds: string[],
): SplitNode {
  const cloned = cloneSplitNode(tree);
  const leaves = collectLeaves(cloned);
  leaves.forEach((leafNode, i) => {
    if (leafNode.subGrid?.enabled) return;
    leafNode.panelId = panelIds[i] ?? null;
  });
  return cloned;
}

export function fillEmptyLeaves(
  tree: SplitNode,
  panelIds: string[],
): SplitNode {
  const cloned = cloneSplitNode(tree);
  const leaves = collectLeaves(cloned);
  const assigned = new Set<string>();
  for (const leafNode of leaves) {
    if (leafNode.panelId) assigned.add(leafNode.panelId);
    if (leafNode.subGrid?.enabled) {
      for (const id of leafNode.subGrid.openPanelIds) assigned.add(id);
    }
  }
  const pending = panelIds.filter((id) => !assigned.has(id));
  let pi = 0;
  for (const leafNode of leaves) {
    if (leafNode.subGrid?.enabled) continue;
    if (leafNode.panelId) continue;
    if (pi < pending.length) {
      leafNode.panelId = pending[pi];
      pi += 1;
    }
  }
  return cloned;
}

/** Split leaves until every open panel has a slot (split canvas mode). */
export function ensureOpenPanelsInTree(
  tree: SplitNode,
  openPanelIds: string[],
): SplitNode {
  let next = tree;
  while (true) {
    const placed = new Set(allPanelIdsInTree(next));
    const missing = openPanelIds.filter((id) => !placed.has(id));
    if (missing.length === 0) break;

    const leaves = collectLeaves(next);
    const target =
      leaves.find((l) => !l.subGrid?.enabled && l.panelId) ??
      leaves.find((l) => !l.subGrid?.enabled);
    if (!target) break;

    next = splitLeaf(next, target.id, "horizontal");
    next = fillEmptyLeaves(next, openPanelIds);
  }
  return next;
}

export function findSplitContainer(
  root: SplitNode,
  id: string,
): SplitContainer | null {
  if (root.type === "leaf") return null;
  if (root.id === id) return root;
  for (const child of root.children) {
    const found = findSplitContainer(child.node, id);
    if (found) return found;
  }
  return null;
}

export function findLeaf(root: SplitNode, id: string): SplitLeaf | null {
  if (root.type === "leaf") return root.id === id ? root : null;
  for (const child of root.children) {
    const found = findLeaf(child.node, id);
    if (found) return found;
  }
  return null;
}

function paneTrackSizePx(containerSizePx: number, paneCount: number): number {
  const sashCount = Math.max(0, paneCount - 1);
  return containerSizePx - sashCount * SASH_THICKNESS_PX;
}

function minPaneRatio(trackSizePx: number, direction?: SplitDirection): number {
  if (trackSizePx <= 0) return 0.08;
  const minPx =
    direction === "horizontal" ? PANEL_MIN_WIDTH_PX : PANEL_MIN_HEIGHT_PX;
  return Math.min(minPx / trackSizePx, 0.08);
}

/** Drag sash `sashIndex` (between child i and i+1) by `deltaPx` along the container axis. */
export function dragSash(
  container: SplitContainer,
  sashIndex: number,
  deltaPx: number,
  containerSizePx: number,
): SplitContainer {
  if (sashIndex < 0 || sashIndex >= container.children.length - 1)
    return container;
  const track = paneTrackSizePx(containerSizePx, container.children.length);
  if (track <= 0) return container;

  const minRatio = minPaneRatio(track, container.direction);
  const next = cloneSplitNode(container) as SplitContainer;
  const left = next.children[sashIndex];
  const right = next.children[sashIndex + 1];
  const deltaRatio = deltaPx / track;
  let newLeft = left.ratio + deltaRatio;
  let newRight = right.ratio - deltaRatio;

  if (newLeft < minRatio) {
    newRight -= minRatio - newLeft;
    newLeft = minRatio;
  }
  if (newRight < minRatio) {
    newLeft -= minRatio - newRight;
    newRight = minRatio;
  }

  left.ratio = Math.max(minRatio, newLeft);
  right.ratio = Math.max(minRatio, newRight);
  next.children = normalizeChildren(next.children);
  return next;
}

/** Read rendered pane sizes back into split ratios (accounts for fixed sash width). */
export function syncContainerRatiosFromDom(
  containerEl: HTMLElement,
  container: SplitContainer,
): SplitContainer {
  const isHorizontal = container.direction === "horizontal";
  const panes = Array.from(
    containerEl.querySelectorAll<HTMLElement>(":scope > .split-pane"),
  );
  if (panes.length !== container.children.length) return container;

  const rect = containerEl.getBoundingClientRect();
  const total = isHorizontal ? rect.width : rect.height;
  const track = paneTrackSizePx(total, container.children.length);
  if (track <= 0) return container;

  const sizes = panes.map((pane) => {
    const pr = pane.getBoundingClientRect();
    return isHorizontal ? pr.width : pr.height;
  });

  const next = cloneSplitNode(container) as SplitContainer;
  next.children = sizes.map((sizePx, i) => ({
    node: next.children[i].node,
    ratio: Math.max(0.01, sizePx / track),
  }));
  next.children = normalizeChildren(next.children);
  return next;
}

/** Local px from container origin to the leading edge of sash `sashIndex`. */
export function sashBoundaryLocalPx(
  containerEl: HTMLElement,
  sashIndex: number,
  isHorizontal: boolean,
): number {
  const panes = Array.from(
    containerEl.querySelectorAll<HTMLElement>(":scope > .split-pane"),
  );
  const containerRect = containerEl.getBoundingClientRect();
  const leftPane = panes[sashIndex];
  if (!leftPane) return 0;
  const pr = leftPane.getBoundingClientRect();
  return isHorizontal
    ? pr.right - containerRect.left
    : pr.bottom - containerRect.top;
}

/** Set sash boundary by absolute local px (leading edge of sash). */
export function setSashBoundary(
  container: SplitContainer,
  sashIndex: number,
  boundaryLocalPx: number,
  containerSizePx: number,
): SplitContainer {
  if (sashIndex < 0 || sashIndex >= container.children.length - 1)
    return container;

  const track = paneTrackSizePx(containerSizePx, container.children.length);
  if (track <= 0) return container;

  const minRatio = minPaneRatio(track, container.direction);
  const next = cloneSplitNode(container) as SplitContainer;
  const fixedBefore = next.children
    .slice(0, sashIndex)
    .reduce((sum, child) => sum + child.ratio, 0);
  const fixedAfter = next.children
    .slice(sashIndex + 2)
    .reduce((sum, child) => sum + child.ratio, 0);

  let targetCumulative =
    (boundaryLocalPx - sashIndex * SASH_THICKNESS_PX) / track;
  targetCumulative = Math.max(
    fixedBefore + minRatio,
    Math.min(1 - fixedAfter - minRatio, targetCumulative),
  );

  const newLeft = targetCumulative - fixedBefore;
  const newRight = 1 - fixedBefore - newLeft - fixedAfter;
  next.children[sashIndex].ratio = Math.max(minRatio, newLeft);
  next.children[sashIndex + 1].ratio = Math.max(minRatio, newRight);
  next.children = normalizeChildren(next.children);
  return next;
}

export function replaceSplitContainer(
  root: SplitNode,
  containerId: string,
  replacement: SplitContainer,
): SplitNode {
  const replace = (node: SplitNode): SplitNode => {
    if (node.type === "leaf") return node;
    if (node.id === containerId) return replacement;
    return {
      ...node,
      children: node.children.map((child) => ({
        node: replace(child.node),
        ratio: child.ratio,
      })),
    };
  };
  return replace(root);
}

/** Split a leaf horizontally (columns) or vertically (rows). */
export function splitLeaf(
  root: SplitNode,
  leafId: string,
  direction: SplitDirection,
): SplitNode {
  const cloned = cloneSplitNode(root);
  const stamp = Date.now();

  const replace = (node: SplitNode): SplitNode => {
    if (node.type === "leaf") {
      if (node.id !== leafId) return node;
      const a = { ...node, id: `leaf-${stamp}-a` };
      const b = leaf(`leaf-${stamp}-b`);
      b.panelId = null;
      return split(`split-${stamp}`, direction, [
        { node: a, ratio: 0.5 },
        { node: b, ratio: 0.5 },
      ]);
    }
    return {
      ...node,
      children: node.children.map((c) => ({
        node: replace(c.node),
        ratio: c.ratio,
      })),
    };
  };

  return replace(cloned);
}

/** Merge a leaf with its sibling when parent has exactly two children. */
export function mergeLeaf(root: SplitNode, leafId: string): SplitNode {
  const cloned = cloneSplitNode(root);

  const replace = (node: SplitNode): SplitNode | null => {
    if (node.type === "leaf") return node;
    if (node.children.length === 2) {
      const left = node.children[0].node;
      const right = node.children[1].node;
      if (left.type === "leaf" && left.id === leafId) return right;
      if (right.type === "leaf" && right.id === leafId) return left;
    }
    return {
      ...node,
      children: node.children
        .map((c) => {
          const replaced = replace(c.node);
          return replaced ? { node: replaced, ratio: c.ratio } : null;
        })
        .filter((c): c is SplitChild => c !== null),
    };
  };

  const result = replace(cloned);
  return result ?? cloned;
}

export function findLeafOwningPanel(
  tree: SplitNode,
  panelId: string,
): SplitLeaf | null {
  for (const leafNode of collectLeaves(tree)) {
    if (leafNode.panelId === panelId) return leafNode;
    if (
      leafNode.subGrid?.enabled &&
      leafNode.subGrid.openPanelIds.includes(panelId)
    ) {
      return leafNode;
    }
  }
  return null;
}

function clearPanelFromLeaf(leaf: SplitLeaf, panelId: string): void {
  if (leaf.panelId === panelId) leaf.panelId = null;
  if (leaf.subGrid?.enabled) {
    leaf.subGrid.openPanelIds = leaf.subGrid.openPanelIds.filter(
      (id) => id !== panelId,
    );
    delete leaf.subGrid.panelLayouts[panelId];
    // Keep subGrid.enabled — empty grid slots stay droppable until # is toggled off.
  }
}

function defaultSubGridEntry(panelId: string): {
  id: string;
  col: number;
  row: number;
  colSpan: number;
  rowSpan: number;
} {
  return { id: panelId, col: 1, row: 1, colSpan: 4, rowSpan: 3 };
}

/** Ensure every sub-grid open panel has a tile layout after profile load. */
export function ensureSubGridPanelLayouts(tree: SplitNode): SplitNode {
  const cloned = cloneSplitNode(tree);
  for (const leafNode of collectLeaves(cloned)) {
    if (!leafNode.subGrid?.enabled) continue;
    const sub = leafNode.subGrid;
    for (const pid of sub.openPanelIds) {
      if (!sub.panelLayouts[pid]) {
        sub.panelLayouts[pid] = defaultSubGridEntry(pid);
      }
    }
  }
  return cloned;
}

/** Remove closed panels from the tree without filling empty slots. */
export function pruneSplitTreeToOpenPanels(
  tree: SplitNode,
  openPanelIds: string[],
): SplitNode {
  const openSet = new Set(openPanelIds);
  const cloned = cloneSplitNode(tree);

  for (const leafNode of collectLeaves(cloned)) {
    if (leafNode.panelId && !openSet.has(leafNode.panelId)) {
      leafNode.panelId = null;
    }
    if (leafNode.subGrid?.enabled) {
      leafNode.subGrid.openPanelIds = leafNode.subGrid.openPanelIds.filter(
        (id) => openSet.has(id),
      );
      for (const pid of Object.keys(leafNode.subGrid.panelLayouts)) {
        if (!openSet.has(pid)) delete leafNode.subGrid.panelLayouts[pid];
      }
    }
  }

  return cloned;
}

/** Copy sub-grid tile layouts into the global panel layout map. */
export function syncGlobalPanelLayoutsFromSplitTree(
  tree: SplitNode,
  panelLayouts: Record<string, TileLayout>,
): void {
  for (const leafNode of collectLeaves(tree)) {
    if (!leafNode.subGrid?.enabled) continue;
    for (const [pid, layout] of Object.entries(leafNode.subGrid.panelLayouts)) {
      panelLayouts[pid] = { ...layout };
    }
  }
}

/** Strip closed panels and fill empty simple slots when entering layout mode. */
export function reconcileSplitTreeWithOpenPanels(
  tree: SplitNode,
  openPanelIds: string[],
): SplitNode {
  const openSet = new Set(openPanelIds);
  const cloned = cloneSplitNode(tree);

  for (const leafNode of collectLeaves(cloned)) {
    if (leafNode.panelId && !openSet.has(leafNode.panelId)) {
      leafNode.panelId = null;
    }
    if (leafNode.subGrid?.enabled) {
      leafNode.subGrid.openPanelIds = leafNode.subGrid.openPanelIds.filter(
        (id) => openSet.has(id),
      );
      for (const pid of Object.keys(leafNode.subGrid.panelLayouts)) {
        if (!openSet.has(pid)) delete leafNode.subGrid.panelLayouts[pid];
      }
    }
  }

  return fillEmptyLeaves(cloned, openPanelIds);
}

/** Copy global grid tile layouts into sub-grid slots when switching grid → layout. */
export function syncSplitTreeFromGridLayouts(
  tree: SplitNode,
  openPanelIds: string[],
  panelLayouts: Record<string, TileLayout>,
  shell: ShellConfig,
): SplitNode {
  const reconciled = reconcileSplitTreeWithOpenPanels(tree, openPanelIds);

  for (const leafNode of collectLeaves(reconciled)) {
    if (!leafNode.subGrid?.enabled) continue;
    for (const pid of leafNode.subGrid.openPanelIds) {
      const layout = panelLayouts[pid];
      if (layout) {
        leafNode.subGrid.panelLayouts[pid] = { ...layout };
      } else if (!leafNode.subGrid.panelLayouts[pid]) {
        leafNode.subGrid.panelLayouts[pid] = defaultSubGridEntry(pid);
      }
    }
    leafNode.subGrid.cell_pixels = shell.cell_pixels;
    leafNode.subGrid.snap_enabled = shell.snap_enabled;
    leafNode.subGrid.show_grid = shell.show_grid;
  }

  return reconciled;
}

/** Push global shell grid settings into all sub-grid slots. */
export function syncSubGridSettingsFromShell(
  tree: SplitNode,
  shell: ShellConfig,
  prevCellPixels?: number,
): SplitNode {
  const cloned = cloneSplitNode(tree);
  const cellChanged =
    prevCellPixels !== undefined && prevCellPixels !== shell.cell_pixels;

  for (const leafNode of collectLeaves(cloned)) {
    if (!leafNode.subGrid?.enabled) continue;
    const sub = leafNode.subGrid;

    if (cellChanged) {
      const prevMetrics = metricsForCellSize(sub.cell_pixels);
      for (const pid of sub.openPanelIds) {
        const tile = sub.panelLayouts[pid];
        if (tile) {
          sub.panelLayouts[pid] = lockTilePixels(tile, prevMetrics, sub.snap_enabled);
        }
      }
    }

    sub.cell_pixels = shell.cell_pixels;
    sub.snap_enabled = shell.snap_enabled;
    sub.show_grid = shell.show_grid;
  }
  return cloned;
}

/** Move or swap a panel into a target slot (simple leaf or sub-grid). */
export function movePanelToLeaf(
  tree: SplitNode,
  panelId: string,
  targetLeafId: string,
  panelLayouts?: Record<string, TileLayout>,
  /** DOM slot id from drag — overrides tree ownership when they disagree after profile load. */
  sourceLeafId?: string | null,
): SplitNode {
  const cloned = cloneSplitNode(tree);
  const owner = findLeafOwningPanel(cloned, panelId);
  let source = owner;
  if (sourceLeafId) {
    const domLeaf = findLeaf(cloned, sourceLeafId);
    if (domLeaf) {
      if (owner && owner.id !== domLeaf.id) {
        clearPanelFromLeaf(owner, panelId);
      }
      source = domLeaf;
    }
  }
  const target = findLeaf(cloned, targetLeafId);
  if (!target || !source || source.id === target.id) return cloned;

  let displaced: string | null = null;

  if (target.subGrid?.enabled) {
    clearPanelFromLeaf(source, panelId);
    if (!target.subGrid.openPanelIds.includes(panelId)) {
      target.subGrid.openPanelIds.push(panelId);
    }
    const fromSubGrid = source.subGrid?.enabled
      ? source.subGrid.panelLayouts[panelId]
      : undefined;
    if (fromSubGrid) {
      target.subGrid.panelLayouts[panelId] = {
        ...fromSubGrid,
        id: panelId,
        pixelLock: undefined,
        freeX: undefined,
        freeY: undefined,
      };
    } else {
      target.subGrid.panelLayouts[panelId] = defaultSubGridEntry(panelId);
    }
    return cloned;
  }

  // Target is a single-panel (non-grid) slot.
  displaced = target.panelId;
  const leavingSubGridLayout = source.subGrid?.enabled
    ? source.subGrid.panelLayouts[panelId]
    : undefined;
  clearPanelFromLeaf(source, panelId);
  target.panelId = panelId;
  if (panelLayouts && leavingSubGridLayout) {
    panelLayouts[panelId] = { ...leavingSubGridLayout };
  }

  if (displaced && displaced !== panelId) {
    if (source.subGrid?.enabled) {
      if (!source.subGrid.openPanelIds.includes(displaced)) {
        source.subGrid.openPanelIds.push(displaced);
      }
      if (!source.subGrid.panelLayouts[displaced]) {
        source.subGrid.panelLayouts[displaced] = defaultSubGridEntry(displaced);
      }
    } else {
      source.panelId = displaced;
    }
  }

  return cloned;
}

/** Sub-grid slot → single-panel slot: keep one panel here, spill extras to empty leaves. */
export function disableLeafSubGrid(tree: SplitNode, leafId: string): SplitNode {
  const cloned = cloneSplitNode(tree);
  const leaf = findLeaf(cloned, leafId);
  if (!leaf || !leaf.subGrid?.enabled) return cloned;

  const ids = [...leaf.subGrid.openPanelIds];
  leaf.subGrid = undefined;
  leaf.panelId = ids[0] ?? null;

  if (ids.length <= 1) return cloned;

  return fillEmptyLeaves(cloned, ids.slice(1));
}

/** Single-panel slot → sub-grid slot. */
export function enableLeafSubGrid(
  tree: SplitNode,
  leafId: string,
  shell: ShellConfig,
): SplitNode {
  const cloned = cloneSplitNode(tree);
  const leaf = findLeaf(cloned, leafId);
  if (!leaf || leaf.subGrid?.enabled) return cloned;

  const ids: string[] = [];
  if (leaf.panelId) ids.push(leaf.panelId);
  leaf.panelId = null;
  leaf.subGrid = {
    enabled: true,
    cell_pixels: shell.cell_pixels,
    snap_enabled: shell.snap_enabled,
    show_grid: shell.show_grid,
    openPanelIds: ids,
    panelLayouts: Object.fromEntries(
      ids.map((id) => [id, defaultSubGridEntry(id)]),
    ),
  };
  return cloned;
}

export function toggleLeafSubGrid(
  tree: SplitNode,
  leafId: string,
  shell: ShellConfig,
): SplitNode {
  const leaf = findLeaf(tree, leafId);
  if (!leaf) return tree;
  if (leaf.subGrid?.enabled) return disableLeafSubGrid(tree, leafId);
  return enableLeafSubGrid(tree, leafId, shell);
}

/** Copy split-tree panel assignments into grid tile layouts for mode switch. */
export function syncGridLayoutsFromSplitTree(
  tree: SplitNode,
  openPanelIds: string[],
  panelLayouts: Record<string, import("./types").TileLayout>,
  defaultTile: (id: string, index: number) => import("./types").TileLayout,
): {
  openPanelIds: string[];
  panelLayouts: Record<string, import("./types").TileLayout>;
} {
  const fromTree = allPanelIdsInTree(tree);
  const mergedIds = [...openPanelIds];
  for (const id of fromTree) {
    if (!mergedIds.includes(id)) mergedIds.push(id);
  }
  const nextLayouts = { ...panelLayouts };
  for (const leafNode of collectLeaves(tree)) {
    if (leafNode.subGrid?.enabled) {
      for (const [pid, layout] of Object.entries(
        leafNode.subGrid.panelLayouts,
      )) {
        nextLayouts[pid] = { ...layout };
      }
    }
  }
  mergedIds.forEach((id, i) => {
    if (!nextLayouts[id]) nextLayouts[id] = defaultTile(id, i);
  });
  return { openPanelIds: mergedIds, panelLayouts: nextLayouts };
}

export function defaultSplitState(
  preset: BuiltInPreset = "two-columns",
): SplitCanvasState {
  return { preset, tree: buildPreset(preset) };
}

export function allPanelIdsInTree(tree: SplitNode): string[] {
  const ids: string[] = [];
  for (const leafNode of collectLeaves(tree)) {
    if (leafNode.panelId) ids.push(leafNode.panelId);
    if (leafNode.subGrid?.enabled) {
      for (const pid of leafNode.subGrid.openPanelIds) {
        if (!ids.includes(pid)) ids.push(pid);
      }
    }
  }
  return ids;
}

export function dedupeOpenPanelIds(ids: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const id of ids) {
    if (!id || seen.has(id)) continue;
    seen.add(id);
    out.push(id);
  }
  return out;
}

/** Each panel id may appear only once in the split tree (first slot wins). */
export function dedupePanelAssignmentsInTree(tree: SplitNode): SplitNode {
  const cloned = cloneSplitNode(tree);
  const seen = new Set<string>();

  for (const leafNode of collectLeaves(cloned)) {
    if (leafNode.panelId) {
      if (seen.has(leafNode.panelId)) {
        leafNode.panelId = null;
      } else {
        seen.add(leafNode.panelId);
      }
    }
    if (leafNode.subGrid?.enabled) {
      const nextIds: string[] = [];
      for (const id of leafNode.subGrid.openPanelIds) {
        if (seen.has(id)) continue;
        seen.add(id);
        nextIds.push(id);
      }
      leafNode.subGrid.openPanelIds = nextIds;
      for (const pid of Object.keys(leafNode.subGrid.panelLayouts)) {
        if (!nextIds.includes(pid)) delete leafNode.subGrid.panelLayouts[pid];
      }
    }
  }
  return cloned;
}
