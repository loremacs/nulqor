import type { Monitor, PhysicalPosition, PhysicalSize } from "@tauri-apps/api/window";

import type { WindowSnapAnchor } from "./types";

/** Windows snap / shadow can be off by a noticeable margin in physical pixels. */
const SNAP_ABS_TOLERANCE_PX = 28;
const SNAP_RATIO_TOLERANCE = 0.035;

function nearPx(a: number, b: number, tolerance = SNAP_ABS_TOLERANCE_PX): boolean {
  return Math.abs(a - b) <= tolerance;
}

function nearRatio(value: number, target: number, tolerance = SNAP_RATIO_TOLERANCE): boolean {
  if (target === 0) return Math.abs(value) <= tolerance;
  return Math.abs(value - target) / Math.abs(target) <= tolerance;
}

type WorkArea = {
  position: PhysicalPosition;
  size: PhysicalSize;
};

export type SnapGeometry = {
  x: number;
  y: number;
  width: number;
  height: number;
};

function workAreaBounds(workArea: WorkArea): {
  x: number;
  y: number;
  width: number;
  height: number;
  halfWidth: number;
  halfHeight: number;
} {
  const x = workArea.position.x;
  const y = workArea.position.y;
  const width = workArea.size.width;
  const height = workArea.size.height;
  return {
    x,
    y,
    width,
    height,
    halfWidth: Math.round(width / 2),
    halfHeight: Math.round(height / 2),
  };
}

export function detectSnapAnchor(
  outerPos: PhysicalPosition,
  outerSize: PhysicalSize,
  workArea: WorkArea,
  maximized: boolean,
): WindowSnapAnchor {
  const wa = workAreaBounds(workArea);
  const ox = outerPos.x;
  const oy = outerPos.y;
  const ow = outerSize.width;
  const oh = outerSize.height;
  const right = ox + ow;
  const bottom = oy + oh;

  const widthRatio = ow / wa.width;
  const heightRatio = oh / wa.height;
  const leftInset = ox - wa.x;
  const topInset = oy - wa.y;
  const rightInset = wa.x + wa.width - right;
  const bottomInset = wa.y + wa.height - bottom;
  const rightAtMid = nearPx(right, wa.x + wa.halfWidth);
  const leftAtMid = nearPx(ox, wa.x + wa.halfWidth);
  const bottomAtMid = nearPx(bottom, wa.y + wa.halfHeight);
  const topAtMid = nearPx(oy, wa.y + wa.halfHeight);

  if (
    maximized &&
    nearRatio(widthRatio, 1) &&
    nearRatio(heightRatio, 1) &&
    leftInset <= SNAP_ABS_TOLERANCE_PX &&
    topInset <= SNAP_ABS_TOLERANCE_PX
  ) {
    return "maximize";
  }

  const halfWidth = nearRatio(widthRatio, 0.5) || nearPx(ow, wa.halfWidth);
  const halfHeight = nearRatio(heightRatio, 0.5) || nearPx(oh, wa.halfHeight);
  const fullWidth = nearRatio(widthRatio, 1) || nearPx(ow, wa.width);
  const fullHeight = nearRatio(heightRatio, 1) || nearPx(oh, wa.height);

  const anchoredLeft = leftInset <= SNAP_ABS_TOLERANCE_PX;
  const anchoredRight = rightInset <= SNAP_ABS_TOLERANCE_PX;
  const anchoredTop = topInset <= SNAP_ABS_TOLERANCE_PX;
  const anchoredBottom = bottomInset <= SNAP_ABS_TOLERANCE_PX;

  if (anchoredLeft && anchoredTop && rightAtMid && bottomAtMid && halfWidth && halfHeight) {
    return "top-left";
  }
  if (anchoredRight && anchoredTop && leftAtMid && bottomAtMid && halfWidth && halfHeight) {
    return "top-right";
  }
  if (anchoredLeft && anchoredBottom && rightAtMid && topAtMid && halfWidth && halfHeight) {
    return "bottom-left";
  }
  if (anchoredRight && anchoredBottom && leftAtMid && topAtMid && halfWidth && halfHeight) {
    return "bottom-right";
  }

  if (anchoredLeft && rightAtMid && fullHeight && halfWidth) return "left";
  if (anchoredRight && leftAtMid && fullHeight && halfWidth) return "right";
  if (anchoredTop && bottomAtMid && fullWidth && halfHeight) return "top";
  if (anchoredBottom && topAtMid && fullWidth && halfHeight) return "bottom";

  return "free";
}

export function snapGeometryForAnchor(
  anchor: WindowSnapAnchor,
  workArea: WorkArea,
): SnapGeometry | null {
  if (anchor === "free") return null;

  const wa = workAreaBounds(workArea);
  switch (anchor) {
    case "maximize":
      return { x: wa.x, y: wa.y, width: wa.width, height: wa.height };
    case "left":
      return { x: wa.x, y: wa.y, width: wa.halfWidth, height: wa.height };
    case "right":
      return {
        x: wa.x + wa.halfWidth,
        y: wa.y,
        width: wa.width - wa.halfWidth,
        height: wa.height,
      };
    case "top":
      return { x: wa.x, y: wa.y, width: wa.width, height: wa.halfHeight };
    case "bottom":
      return {
        x: wa.x,
        y: wa.y + wa.halfHeight,
        width: wa.width,
        height: wa.height - wa.halfHeight,
      };
    case "top-left":
      return { x: wa.x, y: wa.y, width: wa.halfWidth, height: wa.halfHeight };
    case "top-right":
      return {
        x: wa.x + wa.halfWidth,
        y: wa.y,
        width: wa.width - wa.halfWidth,
        height: wa.halfHeight,
      };
    case "bottom-left":
      return {
        x: wa.x,
        y: wa.y + wa.halfHeight,
        width: wa.halfWidth,
        height: wa.height - wa.halfHeight,
      };
    case "bottom-right":
      return {
        x: wa.x + wa.halfWidth,
        y: wa.y + wa.halfHeight,
        width: wa.width - wa.halfWidth,
        height: wa.height - wa.halfHeight,
      };
    default:
      return null;
  }
}

export function monitorWorkArea(monitor: Monitor): WorkArea {
  return {
    position: monitor.workArea.position,
    size: monitor.workArea.size,
  };
}
