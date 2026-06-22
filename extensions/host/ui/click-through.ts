import { invoke } from "@tauri-apps/api/core";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

const INTERACTIVE_SELECTOR =
  ".menu-bar, .menu-bar-menus, .menu-window-controls, .panel-tile, .panel-resize-handle, .menu-dropdown:not([hidden]), .shell-modal-backdrop, .split-sash, .split-slot-edit-bar, .split-slot-edit-bar button";
const POLL_MS = 16;

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

function isOverInteractive(clientX: number, clientY: number): boolean {
  // Topmost element at the cursor — rect iteration misses z-order after panel
  // reorder (appendChild on drag) and over-counts large tile bounds on macOS.
  for (const el of document.elementsFromPoint(clientX, clientY)) {
    if ((el as HTMLElement).closest(INTERACTIVE_SELECTOR)) {
      return true;
    }
  }
  return false;
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
  // Window metrics change only on resize / DPI change. Cache them so the
  // high-frequency poll does not make an innerSize + scaleFactor IPC round-trip
  // every tick (cursor position is the only value that must be polled live).
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

  // Serialize all setIgnoreCursorEvents IPC calls through a single promise
  // chain.  This prevents the race where a slow `true` IPC completes after a
  // fast `false` IPC, leaving the OS in pass-through while local `ignoring`
  // thinks it is clickable.  Each task checks `ignoring` at execution time;
  // superseded tasks (where `ignoring` no longer matches the queued value)
  // are skipped, so only the last-requested state reaches the OS.
  let ipcChain: Promise<void> = Promise.resolve();

  const setIgnoring = (next: boolean): Promise<void> => {
    if (disposed) return Promise.resolve();
    ignoring = next;
    const intended = next;
    ipcChain = ipcChain.then(async () => {
      if (disposed || ignoring !== intended) return; // superseded
      try {
        await win.setIgnoreCursorEvents(intended);
        if (intended) options.onPassThrough?.();
      } catch (err) {
        console.warn("[click-through] setIgnoreCursorEvents failed:", err);
      }
    });
    return ipcChain;
  };

  const ensureClickable = async (): Promise<void> => {
    await setIgnoring(false);
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

  const apply = async (clientX: number, clientY: number): Promise<void> => {
    if (disposed || !passThroughAllowed()) {
      await ensureClickable();
      return;
    }

    if (!(await isActiveForPassThrough())) {
      await ensureClickable();
      return;
    }

    const interactive = isOverInteractive(clientX, clientY);
    if (
      !interactive &&
      (pointerHeld || Date.now() < passThroughDeferredUntil)
    ) {
      await ensureClickable();
      return;
    }
    await setIgnoring(!interactive);
  };

  const updateFromPoll = async (): Promise<void> => {
    if (disposed || !passThroughAllowed()) {
      await ensureClickable();
      return;
    }

    if (!(await isActiveForPassThrough())) {
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
        await setIgnoring(true);
        return;
      }
    }

    await apply(pos.x, pos.y);
  };

  const onMouseMove = (event: MouseEvent): void => {
    pollFailures = 0;
    void apply(event.clientX, event.clientY);
  };

  const onPointerDown = (): void => {
    pointerHeld = true;
    void ensureClickable();
  };

  const onPointerUp = (): void => {
    pointerHeld = false;
  };

  document.addEventListener("mousemove", onMouseMove, { passive: true });
  document.addEventListener("pointerdown", onPointerDown, true);
  document.addEventListener("pointerup", onPointerUp, true);
  document.addEventListener("pointercancel", onPointerUp, true);

  // macOS + click-through: pre-empt pass-through when entering interactive targets so
  // the first click after a drag is not swallowed by the OS (click-to-activate).
  document.addEventListener(
    "pointerover",
    (event) => {
      const target = event.target as HTMLElement;
      if (target.closest(INTERACTIVE_SELECTOR)) {
        void ensureClickable();
      }
    },
    { passive: true },
  );

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
    void ensureClickable();
  };

  const onWindowBlur = (): void => {
    windowFocused = false;
    pollFailures = 0;
    stopPoll();
    void ensureClickable();
  };

  const onWindowFocus = (): void => {
    windowFocused = true;
    pollFailures = 0;
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
    void ensureClickable();
    return ipcChain;
  };

  const deferPassThrough = (ms: number): void => {
    passThroughDeferredUntil = Math.max(
      passThroughDeferredUntil,
      Date.now() + ms,
    );
    void ensureClickable();
  };

  const dispose = (): void => {
    disposed = true;
    stopPoll();
    focusUnlisten?.();
    metricsUnlisten.forEach((u) => u());
    metricsUnlisten.length = 0;
    document.removeEventListener("mousemove", onMouseMove);
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

  return { refresh, suspend, setEnabled, forceClickable, flush, deferPassThrough, dispose };
}
