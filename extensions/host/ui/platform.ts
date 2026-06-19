/**
 * Lightweight OS detection for platform-specific workarounds.
 *
 * Uses navigator.userAgent — available in all three WebView runtimes:
 *   Windows  → WebView2  (Chromium-based, UA contains "Windows")
 *   macOS    → WKWebView (WebKit-based, UA contains "Macintosh")
 *   Linux    → WebKitGTK (WebKit-based, UA contains "Linux")
 *
 * Results are memoized after first call.
 *
 * Usage pattern for platform-specific code:
 *
 *   import { isWindows } from "./platform";
 *   if (isWindows()) { ... Windows-only workaround ... }
 */

let _isWindows: boolean | null = null;
let _isMacOS: boolean | null = null;

export function isWindows(): boolean {
  if (_isWindows === null) {
    _isWindows = /Windows/i.test(navigator.userAgent);
  }
  return _isWindows;
}

export function isMacOS(): boolean {
  if (_isMacOS === null) {
    _isMacOS = /Macintosh|Mac OS X/i.test(navigator.userAgent);
  }
  return _isMacOS;
}
