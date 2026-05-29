export type PanelMount = {
  mount: (container: HTMLElement) => void;
};

export type ShellConfig = {
  /** Fixed square cell edge length in pixels; does not change when the window resizes. */
  cell_pixels: number;
  /** Arrow/spinner increment for the cell size control (pixels). */
  cell_step: number;
  snap_enabled: boolean;
  show_grid: boolean;
  /** Snap split-pane sashes to align with other dividers in layout mode. */
  sash_snap_enabled: boolean;
  /** Pass clicks on empty canvas to the desktop — fullscreen overlay only; off in windowed mode. */
  click_through: boolean;
  /** Keep the Nulqor window above other applications. */
  always_on_top: boolean;
};

export type PanelMeta = {
  id: string;
  kind: string;
};

export type CanvasConfig = {
  open_panels: string[];
  shell: ShellConfig;
  panels: PanelMeta[];
  enabled_extensions?: string[] | null;
};

export type MenuDock = "top" | "bottom" | "left" | "right";

export type WindowSnapAnchor =
  | "free"
  | "left"
  | "right"
  | "top"
  | "bottom"
  | "top-left"
  | "top-right"
  | "bottom-left"
  | "bottom-right"
  | "maximize";

export type WindowFrameState = {
  mode: "fullscreen" | "windowed";
  width: number;
  height: number;
  /** Outer position in physical pixels; x/y < 0 means center on restore. */
  x: number;
  y: number;
  /** Windows desktop snap layout; restored from monitor work area when set. */
  anchor?: WindowSnapAnchor;
  /** Monitor that held the snap layout (best-effort, for multi-monitor restore). */
  monitorName?: string | null;
};

/** Per-panel tile on the canvas desk. */
export type TileLayout = {
  id: string;
  col: number;
  row: number;
  colSpan: number;
  rowSpan: number;
  /** Pixel position when snap is off (relative to desktop top-left). */
  freeX?: number;
  freeY?: number;
  /** Fixed pixel box; used after grid spacing changes until the panel is moved/resized again. */
  pixelLock?: { left: number; top: number; width: number; height: number };
};

export type {
  CanvasMode,
  CanvasProfile,
  GridCanvasState,
} from "./canvas-profiles";
export type {
  SplitCanvasState,
  BuiltInPreset,
  SplitNode,
} from "./split-layout";

export type PersistedShellState = {
  menuDock: MenuDock;
  shell: ShellConfig;
  /** Canvas layout engine: free grid tiles or split-pane regions. */
  canvasMode: import("./canvas-profiles").CanvasMode;
  panelLayouts: Record<string, TileLayout>;
  openPanelIds: string[];
  split?: import("./split-layout").SplitCanvasState;
  canvasProfiles: (import("./canvas-profiles").CanvasProfile | null)[];
  activeProfileId: string | null;
  layoutEditing: boolean;
  windowFrame?: WindowFrameState;
};

export const STORAGE_KEY = "nulqor-shell-v8";
export const STORAGE_KEY_LEGACY = "nulqor-shell-v7";

export const CELL_PIXELS_MIN = 1;
export const CELL_PIXELS_MAX = 256;
export const CELL_STEP_MIN = 1;
export const CELL_STEP_MAX = 256;

export const DEFAULT_SHELL: ShellConfig = {
  cell_pixels: 64,
  cell_step: 10,
  snap_enabled: true,
  show_grid: true,
  sash_snap_enabled: true,
  click_through: true,
  always_on_top: false,
};

/** Minimum panel tile size so header + body content stay readable. */
export const PANEL_MIN_WIDTH_PX = 200;
export const PANEL_MIN_HEIGHT_PX = 120;

export function clampCellPixels(value: number): number {
  const n = Math.round(value);
  if (!Number.isFinite(n)) return DEFAULT_SHELL.cell_pixels;
  return Math.min(CELL_PIXELS_MAX, Math.max(CELL_PIXELS_MIN, n));
}

export function clampCellStep(value: number): number {
  const n = Math.round(value);
  if (!Number.isFinite(n)) return DEFAULT_SHELL.cell_step;
  return Math.min(CELL_STEP_MAX, Math.max(CELL_STEP_MIN, n));
}

export function cellPixels(shell: ShellConfig): number {
  return clampCellPixels(shell.cell_pixels || DEFAULT_SHELL.cell_pixels);
}
