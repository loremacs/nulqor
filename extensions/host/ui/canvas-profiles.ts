import type { TileLayout, ShellConfig } from "./types";
import {
  assignPanelsToTree,
  allPanelIdsInTree,
  cloneSplitNode,
  dedupeOpenPanelIds,
  dedupePanelAssignmentsInTree,
  defaultSplitState,
  ensureSubGridPanelLayouts,
  pruneSplitTreeToOpenPanels,
  type BuiltInPreset,
  type SplitCanvasState,
  type SplitNode,
  type SplitShellSnapshot,
} from "./split-layout";

export type CanvasMode = "grid" | "split";

export type GridCanvasState = {
  cell_pixels: number;
  cell_step: number;
  snap_enabled: boolean;
  show_grid: boolean;
  panelLayouts: Record<string, TileLayout>;
};

export type CanvasProfile = {
  id: string;
  name: string;
  mode: CanvasMode;
  openPanelIds: string[];
  grid?: GridCanvasState;
  split?: SplitCanvasState;
  createdAt: number;
};

export const MAX_CANVAS_PROFILES = 5;

export function newProfileId(): string {
  return `profile-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function emptyProfileSlotName(index: number): string {
  return `Slot ${index + 1}`;
}

export function captureGridProfile(
  name: string,
  shell: ShellConfig,
  panelLayouts: Record<string, TileLayout>,
  openPanelIds: string[],
): CanvasProfile {
  const ids = dedupeOpenPanelIds(openPanelIds);
  const layouts: Record<string, TileLayout> = {};
  for (const id of ids) {
    if (panelLayouts[id]) layouts[id] = { ...panelLayouts[id] };
  }
  return {
    id: newProfileId(),
    name,
    mode: "grid",
    openPanelIds: ids,
    grid: {
      cell_pixels: shell.cell_pixels,
      cell_step: shell.cell_step,
      snap_enabled: shell.snap_enabled,
      show_grid: shell.show_grid,
      panelLayouts: layouts,
    },
    createdAt: Date.now(),
  };
}

export function captureSplitProfile(
  name: string,
  split: SplitCanvasState,
  shell: ShellConfig,
): CanvasProfile {
  const tree = dedupePanelAssignmentsInTree(split.tree);
  const openPanelIds = dedupeOpenPanelIds(allPanelIdsInTree(tree));
  const shellSnapshot: SplitShellSnapshot = {
    cell_pixels: shell.cell_pixels,
    cell_step: shell.cell_step,
    snap_enabled: shell.snap_enabled,
    show_grid: shell.show_grid,
    sash_snap_enabled: shell.sash_snap_enabled,
  };
  return {
    id: newProfileId(),
    name,
    mode: "split",
    openPanelIds,
    split: {
      preset: split.preset,
      tree: cloneSplitNode(tree),
      shell: shellSnapshot,
    },
    createdAt: Date.now(),
  };
}

export function applyProfileToGrid(
  profile: CanvasProfile,
  shell: ShellConfig,
): {
  shell: ShellConfig;
  panelLayouts: Record<string, TileLayout>;
  openPanelIds: string[];
} | null {
  if (profile.mode !== "grid" || !profile.grid) return null;
  return {
    shell: {
      ...shell,
      cell_pixels: profile.grid.cell_pixels,
      cell_step: profile.grid.cell_step,
      snap_enabled: profile.grid.snap_enabled,
      show_grid: profile.grid.show_grid,
    },
    panelLayouts: { ...profile.grid.panelLayouts },
    openPanelIds: [...profile.openPanelIds],
  };
}

export function applyProfileToSplit(
  profile: CanvasProfile,
): SplitCanvasState | null {
  if (profile.mode !== "split" || !profile.split) return null;
  let tree = dedupePanelAssignmentsInTree(cloneSplitNode(profile.split.tree));
  const openPanelIds = dedupeOpenPanelIds(allPanelIdsInTree(tree));
  tree = ensureSubGridPanelLayouts(pruneSplitTreeToOpenPanels(tree, openPanelIds));
  return {
    preset: profile.split.preset,
    tree,
    shell: profile.split.shell,
  };
}

export function createSplitFromPreset(
  preset: BuiltInPreset,
  openPanelIds: string[],
): SplitCanvasState {
  const base = defaultSplitState(preset);
  return {
    preset,
    tree: assignPanelsToTree(base.tree, openPanelIds),
  };
}

export function upsertProfile(
  profiles: (CanvasProfile | null)[],
  slotIndex: number,
  profile: CanvasProfile,
): (CanvasProfile | null)[] {
  const next = [...profiles];
  while (next.length < MAX_CANVAS_PROFILES) next.push(null);
  next[slotIndex] = profile;
  return next.slice(0, MAX_CANVAS_PROFILES);
}

export function normalizeProfileSlots(
  profiles: (CanvasProfile | null)[] | undefined,
): (CanvasProfile | null)[] {
  const slots: (CanvasProfile | null)[] = profiles ? [...profiles] : [];
  while (slots.length < MAX_CANVAS_PROFILES) slots.push(null);
  return slots.slice(0, MAX_CANVAS_PROFILES);
}

export function findLeafPanelIds(tree: SplitNode): string[] {
  const ids: string[] = [];
  const walk = (node: SplitNode): void => {
    if (node.type === "leaf") {
      if (node.panelId) ids.push(node.panelId);
      return;
    }
    for (const child of node.children) walk(child.node);
  };
  walk(tree);
  return ids;
}
