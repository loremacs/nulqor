import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { STORAGE_KEY, type PersistedShellState, type WindowFrameState } from "./types";

export const DEFAULT_WINDOWED_FRAME: WindowFrameState = {
  mode: "windowed",
  width: 1280,
  height: 720,
  x: -1,
  y: -1,
};

export function loadWindowFrame(): WindowFrameState | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedShellState;
    return parsed.windowFrame ?? null;
  } catch {
    return null;
  }
}

export async function captureWindowFrame(): Promise<WindowFrameState> {
  const win = getCurrentWindow();
  const fullscreen = await win.isFullscreen();
  if (fullscreen) {
    return { mode: "fullscreen", width: 1280, height: 720, x: 0, y: 0 };
  }

  const size = await win.innerSize();
  const scale = await win.scaleFactor();
  const logical = size.toLogical(scale);
  const pos = await win.outerPosition();
  return {
    mode: "windowed",
    width: Math.round(logical.width),
    height: Math.round(logical.height),
    x: pos.x,
    y: pos.y,
  };
}

export async function applyWindowFrame(frame: WindowFrameState): Promise<void> {
  const win = getCurrentWindow();
  if (frame.mode === "fullscreen") {
    await win.setResizable(false);
    await win.setFullscreen(true);
    return;
  }

  await win.setFullscreen(false);
  await win.setResizable(true);
  await win.setSize(new LogicalSize(frame.width, frame.height));
  if (frame.x < 0 || frame.y < 0) {
    await win.center();
  } else {
    await win.setPosition(new PhysicalPosition(frame.x, frame.y));
  }
}
