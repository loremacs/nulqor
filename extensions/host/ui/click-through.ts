import { invoke } from "@tauri-apps/api/core";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

const INTERACTIVE_SELECTOR =
  ".menu-bar, .menu-bar-menus, .menu-window-controls, .panel-tile, .menu-dropdown:not([hidden]), .shell-modal-backdrop, .split-sash, .split-slot-edit-bar, .split-slot-edit-bar button";
const POLL_MS = 16;
const HITBOX_PAD_PX = 2;

export type ClickThroughHandle = {
  refresh: () => void;
  suspend: () => () => void;
  setEnabled: (enabled: boolean) => void;
  /** OS-level pass-through off — use when entering windowed mode. */
  forceClickable: () => void;
  dispose: () => void;
};

function isWindowedMode(): boolean {
  const shell = document.querySelector<HTMLElement>(".nulqor-shell");
  return shell?.dataset.windowMode === "windowed";
}

function isOverInteractive(clientX: number, clientY: number): boolean {
  for (const el of document.querySelectorAll<HTMLElement>(
    INTERACTIVE_SELECTOR,
  )) {
    if (el.hidden) continue;
    const rect = el.getBoundingClientRect();
    if (
      clientX >= rect.left - HITBOX_PAD_PX &&
      clientX <= rect.right + HITBOX_PAD_PX &&
      clientY >= rect.top - HITBOX_PAD_PX &&
      clientY <= rect.bottom + HITBOX_PAD_PX
    ) {
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

export function mountClickThrough(initialEnabled: boolean): ClickThroughHandle {
  const win = getCurrentWindow();
  let enabled = initialEnabled;
  let disposed = false;
  let ignoring = false;
  let suspendCount = 0;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let pollFailures = 0;
  let windowFocused = true;

  const setIgnoring = async (next: boolean): Promise<void> => {
    if (disposed || ignoring === next) return;
    ignoring = next;
    try {
      await win.setIgnoreCursorEvents(next);
    } catch (err) {
      console.warn("[click-through] setIgnoreCursorEvents failed:", err);
    }
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

    const inner = await win.innerSize();
    const scale = await win.scaleFactor();
    const logical = inner.toLogical(scale);
    if (
      pos.x < 0 ||
      pos.y < 0 ||
      pos.x > logical.width ||
      pos.y > logical.height
    ) {
      await setIgnoring(true);
      return;
    }

    await apply(pos.x, pos.y);
  };

  const onMouseMove = (event: MouseEvent): void => {
    pollFailures = 0;
    void apply(event.clientX, event.clientY);
  };

  const onPointerDown = (): void => {
    void ensureClickable();
  };

  document.addEventListener("mousemove", onMouseMove, { passive: true });
  document.addEventListener("pointerdown", onPointerDown, true);

  const refresh = (): void => {
    if (!passThroughAllowed()) {
      void ensureClickable();
      return;
    }
    void updateFromPoll();
  };

  const suspend = (): (() => void) => {
    suspendCount += 1;
    void ensureClickable();
    return () => {
      suspendCount = Math.max(0, suspendCount - 1);
      if (passThroughAllowed()) {
        void updateFromPoll();
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

  const dispose = (): void => {
    disposed = true;
    stopPoll();
    focusUnlisten?.();
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("pointerdown", onPointerDown, true);
    void ensureClickable().catch(() => {});
  };

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

  return { refresh, suspend, setEnabled, forceClickable, dispose };
}
