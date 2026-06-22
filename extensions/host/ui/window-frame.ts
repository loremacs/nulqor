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
import { isWindows } from "./platform";
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
/** Overlay covers monitor work area — outer size can differ slightly from inner. */
const OVERLAY_SIZE_TOLERANCE_PX = 48;
// Windows ignores alpha in setBackgroundColor — never use it for overlay transparency.

type ApplyGeometryOptions = {
  startup?: boolean;
};

async function waitForPaint(frames = 2): Promise<void> {
  for (let i = 0; i < frames; i += 1) {
    // requestAnimationFrame is suppressed on hidden windows on macOS WKWebView,
    // so fall back to setTimeout to avoid hanging before win.show() is called.
    await new Promise<void>((resolve) => {
      const t = setTimeout(() => resolve(), 100);
      requestAnimationFrame(() => { clearTimeout(t); resolve(); });
    });
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
  // -1,-1 is the sentinel for "no saved position". Real second-screen positions
  // can have negative x/y (monitors to the left or above the primary).
  if (frame.width > 0 && frame.height > 0 && !(frame.x === -1 && frame.y === -1)) {
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

async function isMonitorAvailable(name: string): Promise<boolean> {
  const monitors = await availableMonitors();
  return monitors.some((m) => m.name === name);
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

  // -1,-1 is the sentinel for "no saved position, center instead".
  if (target.x === -1 && target.y === -1) {
    return {
      size: new LogicalSize(target.width, target.height),
      position: null,
    };
  }

  // If the position was saved on a named monitor that's no longer connected,
  // the coordinates would be off-screen — center on the current monitor instead.
  if (target.monitorName && !(await isMonitorAvailable(target.monitorName))) {
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
  // Do not clamp by primary-monitor bounds — secondary monitors can have negative
  // x/y (left of / above the primary). Trust the caller to provide a valid position.
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

/** True when the window covers the monitor work area (transparent overlay desk). */
export async function isOverlayFrameActive(
  monitorName?: string | null,
): Promise<boolean> {
  const win = getCurrentWindow();
  if (await win.isFullscreen()) return true;

  const monitor = await resolveMonitor(monitorName);
  if (!monitor) return false;

  const wa = monitorWorkArea(monitor);
  const [pos, size] = await Promise.all([win.outerPosition(), win.outerSize()]);
  const right = pos.x + size.width;
  const bottom = pos.y + size.height;
  const targetRight = wa.position.x + wa.size.width;
  const targetBottom = wa.position.y + wa.size.height;

  return (
    Math.abs(pos.x - wa.position.x) <= POSITION_MATCH_TOLERANCE_PX &&
    Math.abs(pos.y - wa.position.y) <= POSITION_MATCH_TOLERANCE_PX &&
    Math.abs(right - targetRight) <= OVERLAY_SIZE_TOLERANCE_PX &&
    Math.abs(bottom - targetBottom) <= OVERLAY_SIZE_TOLERANCE_PX
  );
}

/** Borderless transparent overlay — avoids Windows WebView2 fullscreen opacity bugs. */
export async function applyOverlayFrame(
  monitorName?: string | null,
): Promise<void> {
  const win = getCurrentWindow();
  if (await win.isFullscreen()) {
    await win.setFullscreen(false);
  }
  await win.setResizable(false);

  const monitor = await resolveMonitor(monitorName);
  if (!monitor) {
    await win.setFullscreen(true);
    await nudgeWindowTransparency();
    return;
  }

  const wa = monitorWorkArea(monitor);
  const scale = await win.scaleFactor();
  await win.setSize(
    new LogicalSize(wa.size.width / scale, wa.size.height / scale),
  );
  await win.setPosition(wa.position);
  await nudgeWindowTransparency();
}

async function frameMatchesApplied(
  frame: WindowFrameState | null,
): Promise<boolean> {
  const win = getCurrentWindow();
  if (!frame || frame.mode === "fullscreen") {
    return isOverlayFrameActive(frame?.monitorName);
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
    await applyOverlayFrame(frame.monitorName);
    return;
  }

  if (await win.isFullscreen()) {
    await win.setFullscreen(false);
  }
  await win.setResizable(true);
  await primeWindowedNativeBackground();

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
  await applyOverlayFrame();
}

/**
 * Ensure the window is visible and its title bar is reachable:
 * - If the window center is not on any monitor, center it.
 * - If the window top is above the work area (behind menu bar / off-screen),
 *   clamp it down to the work area top.
 * Runs at startup and on every windowed-mode transition.
 */
async function ensureWindowOnScreen(): Promise<void> {
  const win = getCurrentWindow();
  const [pos, size, monitors] = await Promise.all([
    win.outerPosition(),
    win.outerSize(),
    availableMonitors(),
  ]);
  if (monitors.length === 0) return;

  const centerX = pos.x + Math.floor(size.width / 2);
  const centerY = pos.y + Math.floor(size.height / 2);

  const host = monitors.find(
    (m) =>
      centerX >= m.position.x &&
      centerX < m.position.x + m.size.width &&
      centerY >= m.position.y &&
      centerY < m.position.y + m.size.height,
  );

  if (!host) {
    await win.center();
    return;
  }

  // Clamp top of window to the work area so the title bar is always reachable.
  const wa = monitorWorkArea(host);
  if (pos.y < wa.position.y) {
    await win.setPosition(new PhysicalPosition(pos.x, wa.position.y));
  }
}

export async function captureWindowFrame(
  previous: WindowFrameState | null = null,
): Promise<WindowFrameState> {
  const win = getCurrentWindow();
  const inOverlay = await isOverlayFrameActive(previous?.monitorName);
  const priorWindowed = getWindowedRestoreTarget(previous);

  if (inOverlay) {
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
  // Detect from actual window position — do NOT prefer stored monitorName here.
  // If the user dragged the window to a different monitor, currentMonitor()
  // reflects that; passing the stored name would return the old monitor instead.
  const monitor =
    (await currentMonitor()) ??
    (await monitorFromPoint(
      pos.x + Math.floor(outerSize.width / 2),
      pos.y + Math.floor(outerSize.height / 2),
    )) ??
    (await primaryMonitor());
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
  await ensureWindowOnScreen();
}

export async function applyWindowedFrame(
  frame: WindowFrameState,
): Promise<void> {
  await applyWindowedGeometry(getWindowedRestoreTarget(frame));
}

export async function applyWindowFrame(frame: WindowFrameState): Promise<void> {
  if (frame.mode === "fullscreen") {
    await applyOverlayFrame(frame.monitorName);
    return;
  }

  await applyWindowedGeometry(getWindowedRestoreTarget(frame));
  await primeWindowedNativeBackground();
}

/** WebView2 on Windows only becomes transparent after a size change (alpha ignored). */
async function nudgeWindowTransparency(): Promise<void> {
  if (!isWindows()) return;
  const win = getCurrentWindow();
  const size = await win.innerSize();
  const scale = await win.scaleFactor();
  const logical = size.toLogical(scale);
  if (logical.width < 4 || logical.height < 4) return;
  await win.setSize(new LogicalSize(logical.width + 1, logical.height));
  await waitForPaint(1);
  await win.setSize(new LogicalSize(logical.width, logical.height));
  await waitForPaint(1);
}

async function primeWindowedNativeBackground(): Promise<void> {
  // Windowed fill is CSS-only (#121216). Native setBackgroundColor cannot be
  // cleared to transparent on Windows (alpha channel ignored).
}

async function primeFullscreenNativeBackground(): Promise<void> {
  await nudgeWindowTransparency();
}

/** Match native window fill to overlay vs windowed shell. */
export async function primeNativeBackgroundForMode(
  mode: WindowFrameState["mode"],
): Promise<void> {
  if (mode === "windowed") {
    await primeWindowedNativeBackground();
  } else {
    await primeFullscreenNativeBackground();
  }
}

/**
 * Apply OS geometry while the window is still hidden.
 * Must run before {@link initShell} so syncUi sees the real fullscreen state.
 */
export async function applyStartupGeometry(
  frame: WindowFrameState | null,
): Promise<void> {
  if (frame) {
    if (!(await frameMatchesApplied(frame))) {
      await applyFrameHidden(frame);
    } else {
      await primeNativeBackgroundForMode(frame.mode);
    }
  } else {
    await applyDefaultFirstRunFrame();
  }
  // Windowed mode only: verify the window actually landed on a visible monitor.
  // Catches stale off-screen coordinates from a disconnected external display.
  if (!frame || frame.mode === "windowed") {
    await ensureWindowOnScreen();
  }
}

/** Show the window and reveal the painted shell. */
export async function showShellWindow(
  frame: WindowFrameState | null,
): Promise<void> {
  const win = getCurrentWindow();
  const windowed = frame?.mode === "windowed";

  if (windowed) {
    await waitForPaint(3);
  }

  if (!(await win.isVisible())) {
    await win.show();
  }

  await waitForPaint(windowed ? 2 : 1);
  markShellVisible();
  if (!windowed) {
    await nudgeWindowTransparency();
  }
}

/**
 * Ensure geometry while hidden, paint the shell, then show once.
 * Rust applies from disk in setup; JS only fills gaps (missing file / stale disk).
 */
export async function revealWindowFrame(
  frame: WindowFrameState | null,
): Promise<void> {
  await applyStartupGeometry(frame);
  await showShellWindow(frame);
}

/** @deprecated Use {@link revealWindowFrame} at startup. */
export async function reassertWindowFrame(
  frame: WindowFrameState,
): Promise<void> {
  if (frame.mode !== "windowed") return;
  await applyWindowedGeometry(getWindowedRestoreTarget(frame));
}
