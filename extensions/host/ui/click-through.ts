import { invoke } from "@tauri-apps/api/core";
import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

const INTERACTIVE_SELECTOR =
  ".menu-bar, .panel-tile, .menu-dropdown:not([hidden]), .shell-modal-backdrop, .split-sash, .split-slot-edit-bar, .split-slot-edit-bar button";
const POLL_MS = 16;
const HITBOX_PAD_PX = 2;

export type ClickThroughHandle = {
  refresh: () => void;
  suspend: () => () => void;
  setEnabled: (enabled: boolean) => void;
  dispose: () => void;
};

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

  const setIgnoring = async (next: boolean): Promise<void> => {
    if (disposed || ignoring === next) return;
    ignoring = next;
    try {
      await win.setIgnoreCursorEvents(next);
    } catch (err) {
      console.warn("[click-through] setIgnoreCursorEvents failed:", err);
    }
  };

  const stopPoll = (): void => {
    if (pollTimer !== null) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  };

  const startPoll = (): void => {
    if (pollTimer !== null || disposed || !enabled) return;
    pollTimer = setInterval(() => {
      void updateFromPoll();
    }, POLL_MS);
  };

  const disablePassThrough = async (): Promise<void> => {
    stopPoll();
    pollFailures = 0;
    await setIgnoring(false);
  };

  const apply = async (clientX: number, clientY: number): Promise<void> => {
    if (disposed || !enabled) {
      await disablePassThrough();
      return;
    }

    if (suspendCount > 0 || !document.hasFocus()) {
      await setIgnoring(false);
      return;
    }

    const interactive = isOverInteractive(clientX, clientY);
    const shouldIgnore = !interactive;
    await setIgnoring(shouldIgnore);
  };

  const updateFromPoll = async (): Promise<void> => {
    const pos = await cursorClientCss();
    if (!pos || disposed) {
      pollFailures += 1;
      // Fail open so the shell stays clickable if cursor polling breaks.
      if (pollFailures >= 2) {
        await setIgnoring(false);
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

  document.addEventListener("mousemove", onMouseMove, { passive: true });

  const refresh = (): void => {
    void updateFromPoll();
  };

  const suspend = (): (() => void) => {
    suspendCount += 1;
    void setIgnoring(false);
    return () => {
      suspendCount = Math.max(0, suspendCount - 1);
      if (suspendCount === 0) {
        void updateFromPoll();
      }
    };
  };

  const setEnabled = (next: boolean): void => {
    enabled = next;
    if (!enabled) {
      void disablePassThrough();
      return;
    }
    void win.setIgnoreCursorEvents(false).then(() => {
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
    void win.setIgnoreCursorEvents(false).catch(() => {});
  };

  void win
    .onFocusChanged(({ payload: focused }) => {
      if (disposed || !enabled) return;
      pollFailures = 0;
      if (focused) {
        void setIgnoring(false);
        startPoll();
        void updateFromPoll();
        return;
      }
      // Release passthrough while unfocused — avoids WebView2 ghosting stale menu
      // chrome at old dock positions when clicking through to another app.
      stopPoll();
      void setIgnoring(false);
    })
    .then((unlisten) => {
      focusUnlisten = unlisten;
    });

  if (enabled) {
    void win.setIgnoreCursorEvents(false).then(() => {
      startPoll();
      void updateFromPoll();
    });
  }

  return { refresh, suspend, setEnabled, dispose };
}
