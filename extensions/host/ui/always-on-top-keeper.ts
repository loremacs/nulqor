import { getCurrentWindow } from "@tauri-apps/api/window";

const REASSERT_MS = 500;

export type AlwaysOnTopKeeper = {
  sync: (active: boolean) => void;
  reassertNow: () => void;
  dispose: () => void;
};

/** Re-pin HWND topmost after click-through hands focus to apps behind the overlay. */
export function mountAlwaysOnTopKeeper(
  shouldKeep: () => boolean,
  reassert: () => Promise<void>,
): AlwaysOnTopKeeper {
  const win = getCurrentWindow();
  let timer: ReturnType<typeof setInterval> | null = null;
  let focusUnlisten: (() => void) | null = null;

  const reassertNow = (): void => {
    if (!shouldKeep()) return;
    void reassert();
  };

  const stop = (): void => {
    if (timer !== null) {
      clearInterval(timer);
      timer = null;
    }
    focusUnlisten?.();
    focusUnlisten = null;
  };

  const sync = (active: boolean): void => {
    stop();
    if (!active) return;

    void win
      .onFocusChanged(({ payload: focused }) => {
        if (!focused) reassertNow();
      })
      .then((unlisten) => {
        focusUnlisten = unlisten;
      });

    timer = setInterval(reassertNow, REASSERT_MS);
    reassertNow();
  };

  return {
    sync,
    reassertNow,
    dispose: stop,
  };
}
