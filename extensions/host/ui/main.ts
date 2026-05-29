import { getCurrentWindow } from "@tauri-apps/api/window";

import { initShell } from "./shell";
import {
  loadWindowFrame,
  markShellVisible,
  primeStartupPresentation,
  revealWindowFrame,
} from "./window-frame";

void (async () => {
  try {
    const frame = loadWindowFrame();
    primeStartupPresentation(frame);
    await initShell();
    await revealWindowFrame(frame);
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
