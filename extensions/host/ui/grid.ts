import type { ShellConfig, MenuDock, TileLayout } from "./types";
import { cellPixels } from "./types";

export type GridMetrics = {
  cols: number;
  rows: number;
  gap: number;
  cellSize: number;
  step: number;
  desktopW: number;
  desktopH: number;
  originX: number;
  originY: number;
};

const GAP = 8;

function deskColumns(desktopW: number, step: number): number {
  return Math.max(1, Math.floor((desktopW + GAP) / step));
}

function deskRows(desktopH: number, step: number): number {
  return Math.max(1, Math.floor((desktopH + GAP) / step));
}

/** Fixed square cells tiled across the full desktop; cell size is independent of window size. */
export function updateGridGeometry(
  shellRoot: HTMLElement,
  desktop: HTMLElement,
  shell: ShellConfig,
): GridMetrics {
  const rect = desktop.getBoundingClientRect();
  const cellSize = cellPixels(shell);
  const step = cellSize + GAP;
  const desktopW = Math.max(0, rect.width);
  const desktopH = Math.max(0, rect.height);
  const cols = deskColumns(desktopW, step);
  const rows = deskRows(desktopH, step);

  shellRoot.style.setProperty("--grid-cols", String(cols));
  shellRoot.style.setProperty("--grid-rows", String(rows));
  shellRoot.style.setProperty("--grid-gap", `${GAP}px`);
  shellRoot.style.setProperty("--cell-size", `${cellSize}px`);

  return {
    cols,
    rows,
    gap: GAP,
    cellSize,
    step,
    desktopW,
    desktopH,
    originX: 0,
    originY: 0,
  };
}

export function applyMenuLayout(shellRoot: HTMLElement, dock: MenuDock): void {
  shellRoot.dataset.menuDock = dock;
  shellRoot.style.setProperty(
    "--menu-bar-thickness",
    dock === "left" || dock === "right" ? "44px" : "36px",
  );
}

export function pointerToGridCell(
  clientX: number,
  clientY: number,
  desktop: HTMLElement,
  metrics: GridMetrics,
): { col: number; row: number } {
  const rect = desktop.getBoundingClientRect();
  const x = Math.max(0, Math.min(metrics.desktopW - 0.001, clientX - rect.left - metrics.originX));
  const y = Math.max(0, Math.min(metrics.desktopH - 0.001, clientY - rect.top - metrics.originY));
  const col = Math.min(metrics.cols, Math.floor(x / metrics.step) + 1);
  const row = Math.min(metrics.rows, Math.floor(y / metrics.step) + 1);
  return { col: Math.max(1, col), row: Math.max(1, row) };
}

/** Outer tile size from grid line to grid line (includes inter-cell gaps). */
export function tilePixelSize(
  tile: { colSpan: number; rowSpan: number },
  metrics: GridMetrics,
): { width: number; height: number } {
  return {
    width: tile.colSpan * metrics.step,
    height: tile.rowSpan * metrics.step,
  };
}

export function tileSnapRect(
  tile: { col: number; row: number; colSpan: number; rowSpan: number },
  metrics: GridMetrics,
): { left: number; top: number; width: number; height: number } {
  const { width, height } = tilePixelSize(tile, metrics);
  return {
    left: (tile.col - 1) * metrics.step,
    top: (tile.row - 1) * metrics.step,
    width,
    height,
  };
}

export function nearestMenuDock(clientX: number, clientY: number): MenuDock {
  const h = window.innerHeight;
  const distTop = clientY;
  const distBottom = h - clientY;
  const distLeft = clientX;
  const distRight = window.innerWidth - clientX;
  const min = Math.min(distTop, distBottom, distLeft, distRight);
  if (min === distTop) return "top";
  if (min === distBottom) return "bottom";
  if (min === distLeft) return "left";
  return "right";
}

export function clampTileToDesk(tile: TileLayout, metrics: GridMetrics): TileLayout {
  const colSpan = Math.min(Math.max(1, tile.colSpan), metrics.cols);
  const rowSpan = Math.min(Math.max(1, tile.rowSpan), metrics.rows);
  const maxCol = Math.max(1, metrics.cols - colSpan + 1);
  const maxRow = Math.max(1, metrics.rows - rowSpan + 1);
  const col = Math.min(Math.max(1, tile.col), maxCol);
  const row = Math.min(Math.max(1, tile.row), maxRow);
  return { ...tile, col, row, colSpan, rowSpan };
}

export function tilePixelRect(
  tile: TileLayout,
  metrics: GridMetrics,
  snapEnabled: boolean,
): { left: number; top: number; width: number; height: number } {
  if (!snapEnabled && tile.freeX !== undefined && tile.freeY !== undefined) {
    const { width, height } = tilePixelSize(tile, metrics);
    return { left: tile.freeX, top: tile.freeY, width, height };
  }
  return tileSnapRect(tile, metrics);
}

/** Map a pixel rectangle to the nearest grid cells on `metrics` without scaling the pixel size. */
export function tileLayoutFromPixelRect(
  rect: { left: number; top: number; width: number; height: number },
  metrics: GridMetrics,
  id: string,
): TileLayout {
  const colSpan = Math.max(1, Math.min(metrics.cols, Math.round(rect.width / metrics.step)));
  const rowSpan = Math.max(1, Math.min(metrics.rows, Math.round(rect.height / metrics.step)));
  const maxCol = Math.max(1, metrics.cols - colSpan + 1);
  const maxRow = Math.max(1, metrics.rows - rowSpan + 1);
  const col = Math.min(maxCol, Math.max(1, Math.floor(rect.left / metrics.step) + 1));
  const row = Math.min(maxRow, Math.max(1, Math.floor(rect.top / metrics.step) + 1));
  return { id, col, row, colSpan, rowSpan };
}

/** Keep panel pixel size/position when the cell size changes; re-align to the new grid. */
export function retilePreservingPixels(
  tile: TileLayout,
  oldMetrics: GridMetrics,
  newMetrics: GridMetrics,
  snapEnabled: boolean,
): TileLayout {
  const rect = tilePixelRect(tile, oldMetrics, snapEnabled);
  const next = tileLayoutFromPixelRect(rect, newMetrics, tile.id);
  if (!snapEnabled) {
    return clampTileToDesk({ ...next, freeX: rect.left, freeY: rect.top }, newMetrics);
  }
  return clampTileToDesk({ ...next, freeX: undefined, freeY: undefined }, newMetrics);
}
