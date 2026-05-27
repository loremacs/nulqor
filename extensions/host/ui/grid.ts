import type { ShellConfig, MenuDock, TileLayout } from "./types";
import { cellPixels, PANEL_MIN_HEIGHT_PX, PANEL_MIN_WIDTH_PX } from "./types";

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
/** Menu bar thickness for every dock edge (top/bottom/left/right). */
export const MENU_BAR_THICKNESS_PX = 30;

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
  /** Where grid CSS vars are written (defaults to shell root; sub-grids pass their host). */
  cssScope: HTMLElement = shellRoot,
): GridMetrics {
  const rect = desktop.getBoundingClientRect();
  const cellSize = cellPixels(shell);
  const step = cellSize + GAP;
  const desktopW = Math.max(0, rect.width);
  const desktopH = Math.max(0, rect.height);
  const cols = deskColumns(desktopW, step);
  const rows = deskRows(desktopH, step);

  cssScope.style.setProperty("--grid-cols", String(cols));
  cssScope.style.setProperty("--grid-rows", String(rows));
  cssScope.style.setProperty("--grid-gap", `${GAP}px`);
  cssScope.style.setProperty("--cell-size", `${cellSize}px`);

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
    `${MENU_BAR_THICKNESS_PX}px`,
  );
}

export function pointerToGridCell(
  clientX: number,
  clientY: number,
  desktop: HTMLElement,
  metrics: GridMetrics,
): { col: number; row: number } {
  const rect = desktop.getBoundingClientRect();
  const x = Math.max(
    0,
    Math.min(metrics.desktopW - 0.001, clientX - rect.left - metrics.originX),
  );
  const y = Math.max(
    0,
    Math.min(metrics.desktopH - 0.001, clientY - rect.top - metrics.originY),
  );
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

/** Distance from viewport edge before dock snap preview appears (magnetic field). */
export const MENU_DOCK_SNAP_ZONE_PX = 72;

export function menuBarThicknessPx(_dock: MenuDock): number {
  return MENU_BAR_THICKNESS_PX;
}

/** Nearest dock edge when pointer is within the snap zone; otherwise null. */
export function menuDockSnapTarget(
  clientX: number,
  clientY: number,
): MenuDock | null {
  const h = window.innerHeight;
  const w = window.innerWidth;
  const zone = MENU_DOCK_SNAP_ZONE_PX;
  const distTop = clientY;
  const distBottom = h - clientY;
  const distLeft = clientX;
  const distRight = w - clientX;
  const min = Math.min(distTop, distBottom, distLeft, distRight);
  if (min > zone) return null;
  return nearestMenuDock(clientX, clientY);
}

export function minTileColSpan(metrics: GridMetrics): number {
  return Math.max(1, Math.ceil(PANEL_MIN_WIDTH_PX / metrics.step));
}

export function minTileRowSpan(metrics: GridMetrics): number {
  return Math.max(1, Math.ceil(PANEL_MIN_HEIGHT_PX / metrics.step));
}

export function clampTileToDesk(
  tile: TileLayout,
  metrics: GridMetrics,
): TileLayout {
  const minCols = minTileColSpan(metrics);
  const minRows = minTileRowSpan(metrics);
  const colSpan = Math.min(Math.max(minCols, tile.colSpan), metrics.cols);
  const rowSpan = Math.min(Math.max(minRows, tile.rowSpan), metrics.rows);
  const maxCol = Math.max(1, metrics.cols - colSpan + 1);
  const maxRow = Math.max(1, metrics.rows - rowSpan + 1);
  const col = Math.min(Math.max(1, tile.col), maxCol);
  const row = Math.min(Math.max(1, tile.row), maxRow);
  return { ...tile, col, row, colSpan, rowSpan };
}

export function tileDisplayRect(
  tile: TileLayout,
  metrics: GridMetrics,
  snapEnabled: boolean,
): { left: number; top: number; width: number; height: number } {
  if (tile.pixelLock) return tile.pixelLock;
  if (!snapEnabled && tile.freeX !== undefined && tile.freeY !== undefined) {
    const { width, height } = tilePixelSize(tile, metrics);
    return { left: tile.freeX, top: tile.freeY, width, height };
  }
  return tileSnapRect(tile, metrics);
}

export function tilePixelRect(
  tile: TileLayout,
  metrics: GridMetrics,
  snapEnabled: boolean,
): { left: number; top: number; width: number; height: number } {
  return tileDisplayRect(tile, metrics, snapEnabled);
}

/** Keep exact pixel size/position when cell size changes; panels re-snap on next drag. */
export function lockTilePixels(
  tile: TileLayout,
  metrics: GridMetrics,
  snapEnabled: boolean,
): TileLayout {
  const rect = tileDisplayRect(tile, metrics, snapEnabled);
  return {
    ...tile,
    pixelLock: {
      left: rect.left,
      top: rect.top,
      width: Math.max(rect.width, PANEL_MIN_WIDTH_PX),
      height: Math.max(rect.height, PANEL_MIN_HEIGHT_PX),
    },
    freeX: undefined,
    freeY: undefined,
  };
}

/** Metrics stub for pixel locking (only step/cellSize affect tile rects). */
export function metricsForCellSize(
  cellSize: number,
  desktopW = 1920,
  desktopH = 1080,
): GridMetrics {
  const step = cellSize + GAP;
  return {
    cols: deskColumns(desktopW, step),
    rows: deskRows(desktopH, step),
    gap: GAP,
    cellSize,
    step,
    desktopW,
    desktopH,
    originX: 0,
    originY: 0,
  };
}

/** After a drag with snap, clear pixel lock and align to the new grid. */
export function snapTileFromPointer(
  tile: TileLayout,
  topLeftX: number,
  topLeftY: number,
  desktop: HTMLElement,
  metrics: GridMetrics,
): TileLayout {
  const cell = pointerToGridCell(topLeftX, topLeftY, desktop, metrics);
  const base = tile.pixelLock ?? tileSnapRect(tile, metrics);
  const colSpan = Math.max(
    1,
    Math.min(metrics.cols, Math.round(base.width / metrics.step)),
  );
  const rowSpan = Math.max(
    1,
    Math.min(metrics.rows, Math.round(base.height / metrics.step)),
  );
  return clampTileToDesk(
    {
      ...tile,
      col: cell.col,
      row: cell.row,
      colSpan,
      rowSpan,
      pixelLock: undefined,
      freeX: undefined,
      freeY: undefined,
    },
    metrics,
  );
}

/** Map a pixel rectangle to the nearest grid cells on `metrics` without scaling the pixel size. */
export function tileLayoutFromPixelRect(
  rect: { left: number; top: number; width: number; height: number },
  metrics: GridMetrics,
  id: string,
): TileLayout {
  const colSpan = Math.max(
    1,
    Math.min(metrics.cols, Math.round(rect.width / metrics.step)),
  );
  const rowSpan = Math.max(
    1,
    Math.min(metrics.rows, Math.round(rect.height / metrics.step)),
  );
  const maxCol = Math.max(1, metrics.cols - colSpan + 1);
  const maxRow = Math.max(1, metrics.rows - rowSpan + 1);
  const col = Math.min(
    maxCol,
    Math.max(1, Math.floor(rect.left / metrics.step) + 1),
  );
  const row = Math.min(
    maxRow,
    Math.max(1, Math.floor(rect.top / metrics.step) + 1),
  );
  return { id, col, row, colSpan, rowSpan };
}

type PixelRect = { left: number; top: number; width: number; height: number };

/** Tile bounds in window (viewport) coordinates. */
export function tileWindowRect(
  tile: TileLayout,
  metrics: GridMetrics,
  snapEnabled: boolean,
  desktopRect: DOMRect,
): PixelRect {
  const local = tileDisplayRect(tile, metrics, snapEnabled);
  return {
    left: desktopRect.left + local.left,
    top: desktopRect.top + local.top,
    width: local.width,
    height: local.height,
  };
}

/** Re-map a tile after the work area moves so its window position stays fixed. */
export function tileFromWindowRect(
  windowRect: PixelRect,
  desktopRect: DOMRect,
  metrics: GridMetrics,
  tile: TileLayout,
  snapEnabled: boolean,
): TileLayout {
  const local: PixelRect = {
    left: windowRect.left - desktopRect.left,
    top: windowRect.top - desktopRect.top,
    width: windowRect.width,
    height: windowRect.height,
  };

  if (snapEnabled) {
    return clampTileToDesk(
      {
        ...tileLayoutFromPixelRect(local, metrics, tile.id),
        pixelLock: undefined,
        freeX: undefined,
        freeY: undefined,
      },
      metrics,
    );
  }

  const colSpan = Math.max(
    1,
    Math.min(metrics.cols, Math.round(local.width / metrics.step)),
  );
  const rowSpan = Math.max(
    1,
    Math.min(metrics.rows, Math.round(local.height / metrics.step)),
  );
  return clampTileToDesk(
    {
      ...tile,
      colSpan,
      rowSpan,
      freeX: local.left,
      freeY: local.top,
      pixelLock: undefined,
    },
    metrics,
  );
}
