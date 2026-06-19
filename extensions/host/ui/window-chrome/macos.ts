import type {
  WindowChromeContext,
  WindowChromeHandle,
  WindowChromeStrategy,
} from "./types";
import { isDefaultMenuDragBlocked, mountSharedChrome } from "./chrome-mount";

const UI = {
  restoreButtonTitleFullscreen: "Exit full screen",
  restoreButtonTitleWindowed: "Enter full screen",
  restoreButtonLabelFullscreen: "↙",
  restoreButtonLabelWindowed: "↗",
  brandTitleFullscreen: "Drag to move menu bar · double-click to exit full screen",
  brandTitleWindowed: "Drag to move window · double-click for full screen",
} as const;

function mount(ctx: WindowChromeContext): WindowChromeHandle {
  return mountSharedChrome(
    ctx,
    macosWindowChrome.platform,
    macosWindowChrome.layout,
    UI,
    isDefaultMenuDragBlocked,
  );
}

export const macosWindowChrome: WindowChromeStrategy = {
  platform: "macos",
  layout: { controlsPosition: "leading" },
  ui: UI,
  isMenuDragBlocked: isDefaultMenuDragBlocked,
  mount,
};
