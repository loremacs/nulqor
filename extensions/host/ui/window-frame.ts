import { invoke } from "@tauri-apps/api/core";
import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";
import {
  availableMonitors,
  currentMonitor,
  getCurrentWindow,
  monitorFromPoint,
  primaryMonitor,
  type Monitor,
} from "@tauri-apps/api/window";

import {
  STORAGE_KEY,
  type PersistedShellState,
  type WindowFrameState,
  type WindowSnapAnchor,
} from "./types";
import {
  detectSnapAnchor,
  monitorWorkArea,
  snapGeometryForAnchor,
  type SnapGeometry,
} from "./window-snap";

export const DEFAULT_WINDOWED_FRAME: WindowFrameState = {
  mode: "windowed",
  width: 1280,
  height: 720,
  x: -1,
  y: -1,
  anchor: "free",
};

const POSITION_MATCH_TOLERANCE_PX = 28;
const SIZE_MATCH_TOLERANCE_PX = 4;
const WINDOWED_NATIVE_BG = { red: 18, green: 18, blue: 22, alpha: 255 };

type ApplyGeometryOptions = {
  startup?: boolean;
};

async function waitForPaint(frames = 2): Promise<void> {
  for (let i = 0; i < frames; i += 1) {
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => resolve()),
    );
  }
}

export function primeStartupPresentation(frame: WindowFrameState | null): void {
  if (frame?.mode === "windowed") {
    document.documentElement.classList.add("nulqor-startup-windowed");
  }
}

export function markShellVisible(): void {
  document.documentElement.classList.remove("nulqor-startup-hidden");
  document.documentElement.classList.remove("nulqor-startup-windowed");
  const mode = document.documentElement.dataset.nulqorWindowMode;
  if (mode === "windowed") {
    const bg = "#121216";
    document.documentElement.style.backgroundColor = bg;
    document.body.style.backgroundColor = bg;
    const app = document.getElementById("app");
    if (app) app.style.backgroundColor = bg;
  } else {
    document.documentElement.style.backgroundColor = "transparent";
    document.body.style.backgroundColor = "transparent";
    const app = document.getElementById("app");
    if (app) app.style.backgroundColor = "transparent";
  }
}

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

/** Write frame to disk so Rust can apply geometry before the webview paints. */
export async function syncWindowFrameToDisk(
  frame: WindowFrameState,
): Promise<void> {
  try {
    await invoke("sync_window_frame", { frame });
  } catch (err) {
    console.warn("[window-frame] disk sync failed:", err);
  }
}

/** Windowed geometry to use for restore-down and cold start (even if `mode` is fullscreen). */
export function getWindowedRestoreTarget(
  frame: WindowFrameState | null | undefined,
): WindowFrameState {
  if (!frame) return { ...DEFAULT_WINDOWED_FRAME };
  if (frame.mode === "windowed") return frame;
  if (frame.width > 0 && frame.height > 0 && frame.x >= 0 && frame.y >= 0) {
    return {
      mode: "windowed",
      width: frame.width,
      height: frame.height,
      x: frame.x,
      y: frame.y,
      anchor: frame.anchor,
      monitorName: frame.monitorName,
    };
  }
  return { ...DEFAULT_WINDOWED_FRAME };
}

async function resolveMonitor(
  preferredName: string | null | undefined,
): Promise<Monitor | null> {
  const win = getCurrentWindow();
  if (preferredName) {
    const monitors = await availableMonitors();
    const named = monitors.find((m) => m.name === preferredName);
    if (named) return named;
  }

  const current = await currentMonitor();
  if (current) return current;

  const pos = await win.outerPosition();
  const size = await win.outerSize();
  const fromPoint = await monitorFromPoint(
    pos.x + Math.floor(size.width / 2),
    pos.y + Math.floor(size.height / 2),
  );
  if (fromPoint) return fromPoint;

  return primaryMonitor();
}

async function resolveTargetGeometry(
  target: WindowFrameState,
): Promise<{ size: LogicalSize; position: PhysicalPosition | null }> {
  const win = getCurrentWindow();
  const monitor = await resolveMonitor(target.monitorName);
  const anchor = target.anchor ?? "free";
  const snap: SnapGeometry | null =
    monitor && anchor !== "free"
      ? snapGeometryForAnchor(anchor, monitorWorkArea(monitor))
      : null;

  if (snap) {
    const scale = await win.scaleFactor();
    const width =
      target.width > 0 ? target.width : Math.round(snap.width / scale);
    const height =
      target.height > 0 ? target.height : Math.round(snap.height / scale);
    return {
      size: new LogicalSize(width, height),
      position: new PhysicalPosition(snap.x, snap.y),
    };
  }

  if (target.x < 0 || target.y < 0) {
    return {
      size: new LogicalSize(target.width, target.height),
      position: null,
    };
  }

  return {
    size: new LogicalSize(target.width, target.height),
    position: new PhysicalPosition(target.x, target.y),
  };
}

async function setWindowPosition(
  position: PhysicalPosition,
  options?: ApplyGeometryOptions,
): Promise<void> {
  const win = getCurrentWindow();
  await win.setPosition(position);
  if (options?.startup) {
    await win.setPosition(position);
    return;
  }
  await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  await win.setPosition(position);
}

async function windowFrameMatches(target: WindowFrameState): Promise<boolean> {
  const geometry = await resolveTargetGeometry(target);
  if (!geometry.position) return true;

  const pos = await getCurrentWindow().outerPosition();
  return (
    Math.abs(pos.x - geometry.position.x) <= POSITION_MATCH_TOLERANCE_PX &&
    Math.abs(pos.y - geometry.position.y) <= POSITION_MATCH_TOLERANCE_PX
  );
}

async function frameMatchesApplied(
  frame: WindowFrameState | null,
): Promise<boolean> {
  const win = getCurrentWindow();
  if (!frame || frame.mode === "fullscreen") {
    return await win.isFullscreen();
  }

  if (await win.isFullscreen()) return false;

  const target = getWindowedRestoreTarget(frame);
  const geometry = await resolveTargetGeometry(target);
  const size = await win.innerSize();
  const scale = await win.scaleFactor();
  const logical = size.toLogical(scale);

  const widthOk =
    Math.abs(logical.width - geometry.size.width) <= SIZE_MATCH_TOLERANCE_PX;
  const heightOk =
    Math.abs(logical.height - geometry.size.height) <= SIZE_MATCH_TOLERANCE_PX;
  if (!widthOk || !heightOk) return false;

  return windowFrameMatches(target);
}

/** Apply geometry while the window is still hidden (fallback when Rust file was missing). */
async function applyFrameHidden(frame: WindowFrameState): Promise<void> {
  const win = getCurrentWindow();
  if (frame.mode === "fullscreen") {
    if (!(await win.isFullscreen())) {
      await win.setResizable(false);
      await win.setFullscreen(true);
    }
    return;
  }

  if (await win.isFullscreen()) {
    await win.setFullscreen(false);
  }
  await win.setResizable(true);

  const geometry = await resolveTargetGeometry(getWindowedRestoreTarget(frame));
  await win.setSize(geometry.size);
  if (!geometry.position) {
    await win.center();
    return;
  }
  await setWindowPosition(geometry.position, { startup: true });
}

/** Default first-run overlay when nothing is persisted yet. */
async function applyDefaultFirstRunFrame(): Promise<void> {
  const win = getCurrentWindow();
  if (!(await win.isFullscreen())) {
    await win.setResizable(false);
    await win.setFullscreen(true);
  }
}

export async function captureWindowFrame(
  previous: WindowFrameState | null = null,
): Promise<WindowFrameState> {
  const win = getCurrentWindow();
  const fullscreen = await win.isFullscreen();
  const priorWindowed = getWindowedRestoreTarget(previous);

  if (fullscreen) {
    return {
      mode: "fullscreen",
      width: priorWindowed.width,
      height: priorWindowed.height,
      x: priorWindowed.x,
      y: priorWindowed.y,
      anchor: priorWindowed.anchor,
      monitorName: priorWindowed.monitorName,
    };
  }

  const size = await win.innerSize();
  const scale = await win.scaleFactor();
  const logical = size.toLogical(scale);
  const pos = await win.outerPosition();
  const outerSize = await win.outerSize();
  const maximized = await win.isMaximized();
  const monitor = await resolveMonitor(previous?.monitorName);
  let anchor: WindowSnapAnchor = "free";
  if (monitor) {
    anchor = detectSnapAnchor(
      pos,
      outerSize,
      monitorWorkArea(monitor),
      maximized,
    );
  }

  const captured: WindowFrameState = {
    mode: "windowed",
    width: Math.round(logical.width),
    height: Math.round(logical.height),
    x: pos.x,
    y: pos.y,
    anchor,
    monitorName: monitor?.name ?? previous?.monitorName,
  };
  await syncWindowFrameToDisk(captured);
  return captured;
}

async function applyWindowedGeometry(
  target: WindowFrameState,
  options?: ApplyGeometryOptions,
): Promise<void> {
  const win = getCurrentWindow();
  if (await win.isFullscreen()) {
    await win.setFullscreen(false);
  }
  await win.setResizable(true);

  const geometry = await resolveTargetGeometry(target);
  await win.setSize(geometry.size);
  if (!geometry.position) {
    await win.center();
    return;
  }
  await setWindowPosition(geometry.position, options);
}

export async function applyWindowedFrame(
  frame: WindowFrameState,
): Promise<void> {
  await applyWindowedGeometry(getWindowedRestoreTarget(frame));
}

export async function applyWindowFrame(frame: WindowFrameState): Promise<void> {
  const win = getCurrentWindow();
  if (frame.mode === "fullscreen") {
    if (!(await win.isFullscreen())) {
      await win.setResizable(false);
      await win.setFullscreen(true);
    }
    return;
  }

  await applyWindowedGeometry(getWindowedRestoreTarget(frame));
}

async function primeWindowedNativeBackground(): Promise<void> {
  try {
    await getCurrentWindow().setBackgroundColor(WINDOWED_NATIVE_BG);
  } catch (err) {
    console.warn("[window-frame] setBackgroundColor failed:", err);
  }
}

/**
 * Ensure geometry while hidden, paint the shell, then show once.
 * Rust applies from disk in setup; JS only fills gaps (missing file / stale disk).
 */
export async function revealWindowFrame(
  frame: WindowFrameState | null,
): Promise<void> {
  const win = getCurrentWindow();
  const windowed = frame?.mode === "windowed";

  if (frame) {
    if (!(await frameMatchesApplied(frame))) {
      await applyFrameHidden(frame);
    }
  } else {
    await applyDefaultFirstRunFrame();
  }

  if (windowed) {
    await primeWindowedNativeBackground();
    await waitForPaint(3);
  }

  if (!(await win.isVisible())) {
    await win.show();
  }

  await waitForPaint(windowed ? 2 : 1);
  markShellVisible();
}

/** @deprecated Use {@link revealWindowFrame} at startup. */
export async function reassertWindowFrame(
  frame: WindowFrameState,
): Promise<void> {
  if (frame.mode !== "windowed") return;
  await applyWindowedGeometry(getWindowedRestoreTarget(frame));
}
