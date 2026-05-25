export type PanelMount = {
  mount: (container: HTMLElement) => void;
};

export type ShellConfig = {
  /** Fixed square cell edge length in pixels; does not change when the window resizes. */
  cell_pixels: number;
  snap_enabled: boolean;
  show_grid: boolean;
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
};

export type PersistedShellState = {
  menuDock: MenuDock;
  shell: ShellConfig;
  /** Last layout for every panel that has been opened (key = panel id). */
  panelLayouts: Record<string, TileLayout>;
  /** Currently open panel ids. */
  openPanelIds: string[];
};

export const STORAGE_KEY = "nulqor-shell-v6";

export const DEFAULT_SHELL: ShellConfig = {
  cell_pixels: 64,
  snap_enabled: true,
  show_grid: true,
};

export function cellPixels(shell: ShellConfig): number {
  return Math.min(256, Math.max(16, shell.cell_pixels || DEFAULT_SHELL.cell_pixels));
}
