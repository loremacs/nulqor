import { invoke } from "@tauri-apps/api/core";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

import { isMacOS } from "./platform";

const INTERACTIVE_SELECTOR =
  ".menu-bar, .menu-bar-menus, .menu-window-controls, .panel-tile, .panel-resize-handle, .menu-dropdown:not([hidden]), .shell-modal-backdrop, .split-sash, .split-slot-edit-bar, .split-slot-edit-bar button";
/** Panel drag handles need extra lead time — pass-through must be off before mousedown. */
const PANEL_DRAG_SELECTOR = ".panel-tile-header, .panel-resize-handle";
const POLL_MS = 16;
/** macOS: delay re-enabling OS pass-through so panel clicks are not stolen by Finder. */
const MACOS_PASS_THROUGH_ENABLE_MS = 180;
const MACOS_PANEL_ARM_MS = 120;
const MACOS_FOCUS_ARM_MS = 200;

export type ClickThroughHandle = {
  refresh: () => void;
  suspend: () => () => void;
  setEnabled: (enabled: boolean) => void;
  /** OS-level pass-through off — use when entering windowed mode. */
  forceClickable: () => void;
  /** Await pending setIgnoreCursorEvents IPC (call before starting drags on macOS). */
  flush: () => Promise<void>;
  /** Briefly block re-enabling pass-through after a drag (macOS click-to-activate). */
  deferPassThrough: (ms: number) => void;
  dispose: () => void;
};

function isWindowedMode(): boolean {
  const shell = document.querySelector<HTMLElement>(".nulqor-shell");
  return shell?.dataset.windowMode === "windowed";
}

function hitAt(clientX: number, clientY: number, selector: string): boolean {
  for (const el of document.elementsFromPoint(clientX, clientY)) {
    if ((el as HTMLElement).closest(selector)) {
      return true;
    }
  }
  return false;
}

function isOverInteractive(clientX: number, clientY: number): boolean {
  return hitAt(clientX, clientY, INTERACTIVE_SELECTOR);
}

function isOverPanelDragHandle(clientX: number, clientY: number): boolean {
  return hitAt(clientX, clientY, PANEL_DRAG_SELECTOR);
}

async function cursorClientCss(): Promise<{ x: number; y: number } | null> {
  const win = getCurrentWindow();
  try {
    const scale = await win.scaleFactor();
    const [cursor, inner] = await Promise.all([
      cursorPosition(),
      win.innerPosition(),
    ]);
    return {
      x: (cursor.x - inner.x) / scale,
      y: (cursor.y - inner.y) / scale,
    };
  } catch {
    // Fallback to the Rust command when the window plugin path is unavailable.
  }

  try {
    const scale = await win.scaleFactor();
    const [px, py] = await invoke<[number, number]>("shell_cursor_client");
    return { x: px / scale, y: py / scale };
  } catch (err) {
    console.warn("[click-through] cursor position failed:", err);
    return null;
  }
}

export type ClickThroughOptions = {
  /** Fired when OS pass-through turns on (click went to the desktop/app behind). */
  onPassThrough?: () => void;
};

export function mountClickThrough(
  initialEnabled: boolean,
  options: ClickThroughOptions = {},
): ClickThroughHandle {
  const win = getCurrentWindow();
  let enabled = initialEnabled;
  let disposed = false;
  let ignoring = false;
  let suspendCount = 0;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let pollFailures = 0;
  let windowFocused = true;
  let pointerHeld = false;
  let passThroughDeferredUntil = 0;
  let passThroughEnableTimer: ReturnType<typeof setTimeout> | null = null;
  let cachedInner: Awaited<ReturnType<typeof win.innerSize>> | null = null;
  let cachedScale = 1;
  const metricsUnlisten: Array<() => void> = [];

  const refreshMetrics = async (): Promise<void> => {
    try {
      cachedInner = await win.innerSize();
      cachedScale = await win.scaleFactor();
    } catch {
      // keep previous values on failure
    }
  };

  let ipcChain: Promise<void> = Promise.resolve();

  const setIgnoring = (next: boolean): Promise<void> => {
    if (disposed) return Promise.resolve();
    ignoring = next;
    const intended = next;
    ipcChain = ipcChain.then(async () => {
      if (disposed || ignoring !== intended) return;
      try {
        await win.setIgnoreCursorEvents(intended);
        if (intended) options.onPassThrough?.();
      } catch (err) {
        console.warn("[click-through] setIgnoreCursorEvents failed:", err);
      }
    });
    return ipcChain;
  };

  const cancelPassThroughEnableTimer = (): void => {
    if (passThroughEnableTimer !== null) {
      clearTimeout(passThroughEnableTimer);
      passThroughEnableTimer = null;
    }
  };

  const ensureClickable = async (): Promise<void> => {
    cancelPassThroughEnableTimer();
    await setIgnoring(false);
  };

  const schedulePassThroughEnable = (): void => {
    if (!isMacOS()) {
      void setIgnoring(true);
      return;
    }
    cancelPassThroughEnableTimer();
    passThroughEnableTimer = setTimeout(() => {
      passThroughEnableTimer = null;
      if (disposed || !passThroughAllowed()) return;
      if (pointerHeld || Date.now() < passThroughDeferredUntil) return;
      void setIgnoring(true);
    }, MACOS_PASS_THROUGH_ENABLE_MS);
  };

  const passThroughAllowed = (): boolean =>
    enabled && !isWindowedMode() && suspendCount === 0;

  const stopPoll = (): void => {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  };

  const startPoll = (): void => {
    if (pollTimer !== null || disposed || !passThroughAllowed()) return;
    pollTimer = setInterval(() => {
      void updateFromPoll();
    }, POLL_MS);
  };

  const disablePassThrough = async (): Promise<void> => {
    stopPoll();
    pollFailures = 0;
    cancelPassThroughEnableTimer();
    await ensureClickable();
  };

  const isActiveForPassThrough = async (): Promise<boolean> => {
    if (!passThroughAllowed()) return false;
    let focused = windowFocused;
    try {
      focused = await win.isFocused();
      windowFocused = focused;
    } catch {
      focused = windowFocused && document.hasFocus();
    }
    if (!focused) return false;
    return document.hasFocus();
  };

  const armInteractiveAt = (clientX: number, clientY: number): void => {
    if (!passThroughAllowed()) return;

    if (isOverInteractive(clientX, clientY)) {
      cancelPassThroughEnableTimer();
      void ensureClickable();

      if (isMacOS()) {
        const armMs = isOverPanelDragHandle(clientX, clientY)
          ? MACOS_PANEL_ARM_MS
          : 60;
        passThroughDeferredUntil = Math.max(
          passThroughDeferredUntil,
          Date.now() + armMs,
        );
        if (isOverPanelDragHandle(clientX, clientY) && !document.hasFocus()) {
          void win.setFocus();
        }
      }
    }
  };

  const apply = async (clientX: number, clientY: number): Promise<void> => {
    if (disposed || !passThroughAllowed()) {
      await ensureClickable();
      return;
    }

    if (!(await isActiveForPassThrough())) {
      cancelPassThroughEnableTimer();
      await ensureClickable();
      return;
    }

    const interactive = isOverInteractive(clientX, clientY);
    if (interactive) {
      armInteractiveAt(clientX, clientY);
      return;
    }

    if (pointerHeld || Date.now() < passThroughDeferredUntil) {
      cancelPassThroughEnableTimer();
      await ensureClickable();
      return;
    }

    schedulePassThroughEnable();
  };

  const updateFromPoll = async (): Promise<void> => {
    if (disposed || !passThroughAllowed()) {
      await ensureClickable();
      return;
    }

    if (!(await isActiveForPassThrough())) {
      cancelPassThroughEnableTimer();
      await ensureClickable();
      return;
    }

    const pos = await cursorClientCss();
    if (!pos) {
      pollFailures += 1;
      if (pollFailures >= 2) {
        await ensureClickable();
      }
      return;
    }

    pollFailures = 0;

    if (!cachedInner) await refreshMetrics();
    if (cachedInner) {
      const logical = cachedInner.toLogical(cachedScale);
      if (
        pos.x < 0 ||
        pos.y < 0 ||
        pos.x > logical.width ||
        pos.y > logical.height
      ) {
        cancelPassThroughEnableTimer();
        await setIgnoring(true);
        return;
      }
    }

    await apply(pos.x, pos.y);
  };

  const onMouseMove = (event: MouseEvent): void => {
    pollFailures = 0;
    armInteractiveAt(event.clientX, event.clientY);
    void apply(event.clientX, event.clientY);
  };

  const onPointerMove = (event: PointerEvent): void => {
    pollFailures = 0;
    armInteractiveAt(event.clientX, event.clientY);
  };

  const onPointerDown = (): void => {
    pointerHeld = true;
    cancelPassThroughEnableTimer();
    void ensureClickable();
  };

  const onPointerUp = (): void => {
    pointerHeld = false;
  };

  document.addEventListener("mousemove", onMouseMove, { passive: true });
  document.addEventListener("pointermove", onPointerMove, {
    passive: true,
    capture: true,
  });
  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("pointerup", onPointerUp, true);
  document.addEventListener("pointercancel", onPointerUp, true);

  const refresh = (): void => {
    if (!passThroughAllowed()) {
      void ensureClickable();
      return;
    }
    void updateFromPoll();
  };

  const suspend = (): (() => void) => {
    suspendCount += 1;
    stopPoll();
    cancelPassThroughEnableTimer();
    void ensureClickable();
    return () => {
      suspendCount = Math.max(0, suspendCount - 1);
      if (passThroughAllowed()) {
        void ensureClickable().then(() => {
          startPoll();
          void updateFromPoll();
        });
      } else {
        void ensureClickable();
      }
    };
  };

  const setEnabled = (next: boolean): void => {
    enabled = next;
    if (!passThroughAllowed()) {
      void disablePassThrough();
      return;
    }
    void ensureClickable().then(() => {
      startPoll();
      void updateFromPoll();
    });
  };

  const forceClickable = (): void => {
    stopPoll();
    pollFailures = 0;
    cancelPassThroughEnableTimer();
    void ensureClickable();
  };

  const onWindowBlur = (): void => {
    windowFocused = false;
    pollFailures = 0;
    stopPoll();
    cancelPassThroughEnableTimer();
    void ensureClickable();
  };

  const onWindowFocus = (): void => {
    windowFocused = true;
    pollFailures = 0;
    cancelPassThroughEnableTimer();
    if (isMacOS()) {
      deferPassThrough(MACOS_FOCUS_ARM_MS);
    }
    if (!passThroughAllowed()) {
      void ensureClickable();
      return;
    }
    void ensureClickable().then(() => {
      startPoll();
      void updateFromPoll();
    });
  };

  let focusUnlisten: (() => void) | null = null;

  const flush = (): Promise<void> => {
    cancelPassThroughEnableTimer();
    void ensureClickable();
    return ipcChain;
  };

  const deferPassThrough = (ms: number): void => {
    passThroughDeferredUntil = Math.max(
      passThroughDeferredUntil,
      Date.now() + ms,
    );
    cancelPassThroughEnableTimer();
    void ensureClickable();
  };

  const dispose = (): void => {
    disposed = true;
    stopPoll();
    cancelPassThroughEnableTimer();
    focusUnlisten?.();
    metricsUnlisten.forEach((u) => u());
    metricsUnlisten.length = 0;
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("pointermove", onPointerMove, true);
    document.removeEventListener("pointerdown", onPointerDown, true);
    document.removeEventListener("pointerup", onPointerUp, true);
    document.removeEventListener("pointercancel", onPointerUp, true);
    void ensureClickable().catch(() => {});
  };

  void refreshMetrics();
  void win
    .onResized(() => {
      void refreshMetrics();
    })
    .then((u) => {
      metricsUnlisten.push(u);
    });
  void win
    .onScaleChanged(() => {
      void refreshMetrics();
    })
    .then((u) => {
      metricsUnlisten.push(u);
    });

  void win
    .onFocusChanged(({ payload: focused }) => {
      if (focused) onWindowFocus();
      else onWindowBlur();
    })
    .then((unlisten) => {
      focusUnlisten = unlisten;
    });

  void win.isFocused().then((focused) => {
    windowFocused = focused;
    if (!focused || isWindowedMode()) {
      void ensureClickable();
    }
  });

  if (passThroughAllowed()) {
    void ensureClickable().then(() => {
      startPoll();
      void updateFromPoll();
    });
  } else {
    void ensureClickable();
  }

  return {
    refresh,
    suspend,
    setEnabled,
    forceClickable,
    flush,
    deferPassThrough,
    dispose,
  };
}
