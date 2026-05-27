import type { WindowFrameState } from "../types";

export type HostPlatform = "windows" | "macos" | "linux" | "unknown";

export type WindowMode = "fullscreen" | "windowed";

/** Static chrome layout hints consumed by CSS (`data-*` on the shell root). */
export type WindowChromeLayout = {
  controlsPosition: "leading" | "trailing";
};

/** Labels and icons for window control buttons and title-bar hints. */
export type WindowChromeUi = {
  restoreButtonTitleFullscreen: string;
  restoreButtonTitleWindowed: string;
  restoreButtonLabelFullscreen: string;
  restoreButtonLabelWindowed: string;
  brandTitleFullscreen: string;
  brandTitleWindowed: string;
};

export type WindowChromeContext = {
  menuBar: HTMLElement;
  shellRoot: HTMLElement;
  restoreBtn: HTMLButtonElement;
  getWindowFrame: () => WindowFrameState;
  refreshWindowFrame: () => Promise<void>;
  onLayoutChanged: () => void;
  /** Fullscreen only: re-dock menu bar after title-bar drag. */
  onMenuDockDrag: (event: PointerEvent) => void;
  /** Fullscreen only: pointer moved while docking drag is active. */
  onMenuDockDragMove?: (event: PointerEvent) => void;
  /** Fullscreen or windowed: drag gesture ended (hide snap preview). */
  onMenuDockDragEnd?: () => void;
  /** Fired when display mode changes (fullscreen overlay vs normal window). */
  onWindowModeChanged?: (mode: WindowMode, previous: WindowMode) => void;
  initialMode: WindowMode;
};

export type WindowChromeHandle = {
  platform: HostPlatform;
  layout: WindowChromeLayout;
  syncUi: () => Promise<void>;
  toggleFullscreen: () => Promise<void>;
  getWindowMode: () => WindowMode;
};

/**
 * OS-specific window chrome: title-bar gestures, control semantics, and layout hints.
 * Canvas/shell behavior (grid, menu dock) stays outside this boundary.
 */
export interface WindowChromeStrategy {
  readonly platform: HostPlatform;
  readonly layout: WindowChromeLayout;
  readonly ui: WindowChromeUi;
  isMenuDragBlocked(target: HTMLElement): boolean;
  mount(ctx: WindowChromeContext): WindowChromeHandle;
}
