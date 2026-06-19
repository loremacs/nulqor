import type {
  WindowChromeContext,
  WindowChromeHandle,
  WindowChromeStrategy,
} from "./types";
import { isDefaultMenuDragBlocked, mountSharedChrome } from "./chrome-mount";

const UI = {
  restoreButtonTitleFullscreen: "Restore down",
  restoreButtonTitleWindowed: "Fullscreen",
  restoreButtonLabelFullscreen: "❐",
  restoreButtonLabelWindowed: "□",
  brandTitleFullscreen: "Drag to move menu bar · double-click to restore",
  brandTitleWindowed: "Drag to move window · double-click for fullscreen",
} as const;

function mount(ctx: WindowChromeContext): WindowChromeHandle {
  return mountSharedChrome(
    ctx,
    windowsWindowChrome.platform,
    windowsWindowChrome.layout,
    UI,
    isDefaultMenuDragBlocked,
  );
}

export const windowsWindowChrome: WindowChromeStrategy = {
  platform: "windows",
  layout: { controlsPosition: "trailing" },
  ui: UI,
  isMenuDragBlocked: isDefaultMenuDragBlocked,
  mount,
};
