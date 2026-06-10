import { getCurrentWindow } from "@tauri-apps/api/window";

import { initShell } from "./shell";
import {
  applyStartupGeometry,
  loadWindowFrame,
  markShellVisible,
  primeStartupPresentation,
  showShellWindow,
} from "./window-frame";

void (async () => {
  try {
    const frame = loadWindowFrame();
    primeStartupPresentation(frame);
    // Geometry before initShell — syncUi must see real fullscreen, not 1×1 off-screen.
    await applyStartupGeometry(frame);
    const shell = await initShell();
    await showShellWindow(frame);
    await shell.resyncWindowMode();
  } catch (err) {
    console.error("[host] shell failed to start", err);
    markShellVisible();
    const app = document.getElementById("app");
    if (app) {
      app.textContent = `Shell error: ${err}`;
    }
    try {
      const win = getCurrentWindow();
      if (!(await win.isVisible())) {
        await win.show();
      }
    } catch (showErr) {
      console.warn("[host] failed to show window:", showErr);
    }
  }
})();
