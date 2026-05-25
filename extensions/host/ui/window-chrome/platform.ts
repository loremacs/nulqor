import type { HostPlatform } from "./types";

/** Runtime OS detection for window-chrome strategy selection (no extra Tauri plugin). */
export function detectHostPlatform(): HostPlatform {
  if (typeof navigator === "undefined") return "unknown";

  const platform = navigator.platform.toLowerCase();
  if (platform.startsWith("win")) return "windows";
  if (platform.startsWith("mac")) return "macos";
  if (platform.includes("linux")) return "linux";

  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("windows")) return "windows";
  if (ua.includes("mac os") || ua.includes("macintosh")) return "macos";
  if (ua.includes("linux")) return "linux";

  return "unknown";
}
