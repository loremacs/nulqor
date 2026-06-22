import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  applyOverlayFrame,
  applyWindowedFrame,
  getWindowedRestoreTarget,
  isOverlayFrameActive,
} from "../window-frame";
import type {
  HostPlatform,
  WindowChromeContext,
  WindowChromeHandle,
  WindowChromeLayout,
  WindowChromeUi,
  WindowMode,
} from "./types";

const TITLE_BAR_DRAG_PX = 5;

export function isDefaultMenuDragBlocked(target: HTMLElement): boolean {
  return Boolean(
    target.closest(
      ".menu-item, .menu-window-btn, .menu-window-controls, .menu-dropdown, .menu-dropdown-row, input, label",
    ),
  );
}

function trackTitleBarDrag(
  pointerDown: PointerEvent,
  windowMode: WindowMode,
  onDock: ((event: PointerEvent) => void) | undefined,
  onDragMove?: (event: PointerEvent) => void,
  onDragEnd?: () => void,
  onNativeDragStart?: () => void,
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
        onNativeDragStart?.();
        return;
      }
    }

    if (dragging && windowMode === "fullscreen") {
      onDragMove?.(event);
    }
  };

  const onUp = (event: PointerEvent): void => {
    if (dragging && windowMode === "fullscreen") {
      onDock?.(event);
    }
    onDragEnd?.();
    cleanup();
  };

  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
  document.addEventListener("pointercancel", onUp);
}

export function mountSharedChrome(
  ctx: WindowChromeContext,
  platform: HostPlatform,
  layout: WindowChromeLayout,
  ui: WindowChromeUi,
  isMenuDragBlockedFn: (target: HTMLElement) => boolean,
): WindowChromeHandle {
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
  // macOS: startDragging() causes the app to lose activation; track so we can restore it.
  let pendingNativeDragRefocus = false;
  let nativeDragRefocusReset: ReturnType<typeof setTimeout> | null = null;

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
      ? ui.restoreButtonTitleFullscreen
      : ui.restoreButtonTitleWindowed;
    restoreBtn.textContent = overlay
      ? ui.restoreButtonLabelFullscreen
      : ui.restoreButtonLabelWindowed;
    const dragRegion = menuBar.querySelector<HTMLElement>(".menu-bar-drag");
    if (dragRegion) {
      dragRegion.title = overlay
        ? ui.brandTitleFullscreen
        : ui.brandTitleWindowed;
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
    if (isMenuDragBlockedFn(target)) return;
    event.preventDefault();
    event.stopPropagation();
    void toggleFullscreen();
  });

  menuBar.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    if (isMenuDragBlockedFn(target)) return;

    const onNativeDragStart = platform === "macos" ? (): void => {
      pendingNativeDragRefocus = true;
      // Safety reset: clear the flag if focus is never lost (e.g. drag cancelled).
      if (nativeDragRefocusReset !== null) clearTimeout(nativeDragRefocusReset);
      nativeDragRefocusReset = setTimeout(() => {
        nativeDragRefocusReset = null;
        pendingNativeDragRefocus = false;
      }, 5000);
    } : undefined;

    trackTitleBarDrag(
      event,
      windowMode,
      onMenuDockDrag,
      onMenuDockDragMove,
      onMenuDockDragEnd,
      onNativeDragStart,
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

  // macOS: restore app activation after a native window drag (startDragging) deactivates us.
  if (platform === "macos") {
    void getCurrentWindow()
      .onFocusChanged(({ payload: focused }) => {
        if (!focused && pendingNativeDragRefocus) {
          pendingNativeDragRefocus = false;
          if (nativeDragRefocusReset !== null) {
            clearTimeout(nativeDragRefocusReset);
            nativeDragRefocusReset = null;
          }
          setTimeout(() => void getCurrentWindow().setFocus(), 50);
        }
      })
      .catch(() => {});
  }

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
    platform,
    layout,
    syncUi,
    toggleFullscreen,
    getWindowMode: () => windowMode,
  };
}
