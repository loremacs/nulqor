import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  applyOverlayFrame,
  applyWindowedFrame,
  getWindowedRestoreTarget,
  isOverlayFrameActive,
} from "../window-frame";
import type {
  WindowChromeContext,
  WindowChromeHandle,
  WindowChromeStrategy,
  WindowMode,
} from "./types";

const TITLE_BAR_DRAG_PX = 5;

const UI = {
  restoreButtonTitleFullscreen: "Restore down",
  restoreButtonTitleWindowed: "Fullscreen",
  restoreButtonLabelFullscreen: "❐",
  restoreButtonLabelWindowed: "□",
  brandTitleFullscreen: "Drag to move menu bar · double-click to restore",
  brandTitleWindowed: "Drag to move window · double-click for fullscreen",
} as const;

function isMenuDragBlocked(target: HTMLElement): boolean {
  return Boolean(
    target.closest(
      ".menu-item, .menu-window-btn, .menu-window-controls, .menu-dropdown, .menu-dropdown-row, input, label",
    ),
  );
}

/**
 * Defer drag until the pointer moves so dblclick is not suppressed by preventDefault.
 * Windowed: startDragging after threshold. Fullscreen: invoke onDock after threshold.
 */
function trackTitleBarDrag(
  pointerDown: PointerEvent,
  windowMode: WindowMode,
  onDock: (event: PointerEvent) => void,
  onDragMove?: (event: PointerEvent) => void,
  onDragEnd?: () => void,
): void {
  const originX = pointerDown.clientX;
  const originY = pointerDown.clientY;
  let dragging = false;

  const cleanup = (): void => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
    document.removeEventListener("pointercancel", onUp);
  };

  const onMove = (event: PointerEvent): void => {
    if (!dragging) {
      const dx = event.clientX - originX;
      const dy = event.clientY - originY;
      if (Math.hypot(dx, dy) < TITLE_BAR_DRAG_PX) return;
      dragging = true;
      if (windowMode === "windowed") {
        cleanup();
        onDragEnd?.();
        void getCurrentWindow().startDragging();
        return;
      }
    }

    if (dragging && windowMode === "fullscreen") {
      onDragMove?.(event);
    }
  };

  const onUp = (event: PointerEvent): void => {
    if (dragging && windowMode === "fullscreen") {
      onDock(event);
    }
    onDragEnd?.();
    cleanup();
  };

  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
  document.addEventListener("pointercancel", onUp);
}

function mount(ctx: WindowChromeContext): WindowChromeHandle {
  const {
    menuBar,
    shellRoot,
    restoreBtn,
    getWindowFrame,
    refreshWindowFrame,
    flushWindowFrame,
    onLayoutChanged,
    onMenuDockDrag,
    onMenuDockDragMove,
    onMenuDockDragEnd,
    onWindowModeChanged,
  } = ctx;
  let windowMode = ctx.initialMode;
  let togglingFullscreen = false;
  let framePersistTimer: ReturnType<typeof setTimeout> | null = null;

  const scheduleWindowFramePersist = (): void => {
    if (framePersistTimer) clearTimeout(framePersistTimer);
    framePersistTimer = setTimeout(() => {
      framePersistTimer = null;
      void refreshWindowFrame();
    }, 180);
  };

  const setWindowMode = (mode: WindowMode): void => {
    if (mode === windowMode) return;
    const previous = windowMode;
    windowMode = mode;
    shellRoot.dataset.windowMode = mode;
    onWindowModeChanged?.(mode, previous);
  };

  const syncUi = async (): Promise<void> => {
    const overlay = await isOverlayFrameActive(getWindowFrame()?.monitorName);
    setWindowMode(overlay ? "fullscreen" : "windowed");
    restoreBtn.title = overlay
      ? UI.restoreButtonTitleFullscreen
      : UI.restoreButtonTitleWindowed;
    restoreBtn.textContent = overlay
      ? UI.restoreButtonLabelFullscreen
      : UI.restoreButtonLabelWindowed;
    const dragRegion = menuBar.querySelector<HTMLElement>(".menu-bar-drag");
    if (dragRegion) {
      dragRegion.title = overlay
        ? UI.brandTitleFullscreen
        : UI.brandTitleWindowed;
    }
  };

  const toggleFullscreen = async (): Promise<void> => {
    if (togglingFullscreen) return;
    togglingFullscreen = true;
    try {
      const inOverlay = await isOverlayFrameActive(
        getWindowFrame()?.monitorName,
      );
      if (inOverlay) {
        setWindowMode("windowed");
        await applyWindowedFrame(getWindowedRestoreTarget(getWindowFrame()));
      } else {
        await refreshWindowFrame();
        setWindowMode("fullscreen");
        await applyOverlayFrame(getWindowFrame()?.monitorName);
      }
      await syncUi();
      await refreshWindowFrame();
      onLayoutChanged();
    } finally {
      togglingFullscreen = false;
    }
  };

  restoreBtn.addEventListener("click", () => {
    void toggleFullscreen();
  });

  menuBar.addEventListener("dblclick", (event) => {
    const target = event.target as HTMLElement;
    if (isMenuDragBlocked(target)) return;
    event.preventDefault();
    event.stopPropagation();
    void toggleFullscreen();
  });

  menuBar.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    if (isMenuDragBlocked(target)) return;

    trackTitleBarDrag(
      event,
      windowMode,
      onMenuDockDrag,
      onMenuDockDragMove,
      onMenuDockDragEnd,
    );
  });

  menuBar.querySelector('[data-action="minimize"]')?.addEventListener("click", () => {
    void getCurrentWindow().minimize();
  });

  menuBar.querySelector('[data-action="close"]')?.addEventListener("click", () => {
    void getCurrentWindow().close();
  });

  void getCurrentWindow()
    .onCloseRequested(async (event) => {
      event.preventDefault();
      if (framePersistTimer) {
        clearTimeout(framePersistTimer);
        framePersistTimer = null;
      }
      await refreshWindowFrame();
      await getCurrentWindow().destroy();
    })
    .catch((err) => {
      console.warn("[window-chrome] onCloseRequested unavailable:", err);
    });

  window.addEventListener("pagehide", () => {
    if (framePersistTimer) {
      clearTimeout(framePersistTimer);
      framePersistTimer = null;
    }
    flushWindowFrame?.();
    void refreshWindowFrame();
  });

  void getCurrentWindow()
    .onMoved(() => {
      scheduleWindowFramePersist();
    })
    .catch((err) => {
      console.warn("[window-chrome] onMoved unavailable:", err);
    });

  void getCurrentWindow()
    .onResized(() => {
      onLayoutChanged();
      void syncUi();
      scheduleWindowFramePersist();
    })
    .catch((err) => {
      console.warn("[window-chrome] onResized unavailable:", err);
    });

  void syncUi().catch((err) => {
    console.warn("[window-chrome] syncUi failed:", err);
  });

  return {
    platform: windowsWindowChrome.platform,
    layout: windowsWindowChrome.layout,
    syncUi,
    toggleFullscreen,
    getWindowMode: () => windowMode,
  };
}

export const windowsWindowChrome: WindowChromeStrategy = {
  platform: "windows",
  layout: { controlsPosition: "trailing" },
  ui: UI,
  isMenuDragBlocked,
  mount,
};
