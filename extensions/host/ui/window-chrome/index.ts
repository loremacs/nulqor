import { detectHostPlatform } from "./platform";
import type { HostPlatform, WindowChromeContext, WindowChromeHandle, WindowChromeStrategy } from "./types";
import { windowsWindowChrome } from "./windows";

const STRATEGIES: Partial<Record<HostPlatform, WindowChromeStrategy>> = {
  windows: windowsWindowChrome,
  // macos: macosWindowChrome — add when shipping macOS (traffic lights, AppleActionOnDoubleClick)
  // linux: linuxWindowChrome — add when shipping Linux (optional WM tweaks)
};

const FALLBACK_STRATEGY = windowsWindowChrome;

export function getWindowChromeStrategy(platform: HostPlatform = detectHostPlatform()): WindowChromeStrategy {
  return STRATEGIES[platform] ?? FALLBACK_STRATEGY;
}

export function mountWindowChrome(ctx: WindowChromeContext): WindowChromeHandle {
  const strategy = getWindowChromeStrategy();
  const handle = strategy.mount(ctx);

  ctx.shellRoot.dataset.platform = strategy.platform;
  ctx.shellRoot.dataset.windowControls = strategy.layout.controlsPosition;

  return handle;
}

export type { HostPlatform, WindowChromeContext, WindowChromeHandle, WindowChromeLayout, WindowChromeStrategy, WindowChromeUi, WindowMode } from "./types";
export { detectHostPlatform } from "./platform";
