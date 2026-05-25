import { getCurrentWindow } from "@tauri-apps/api/window";

import { initShell } from "./shell";
import { applyWindowFrame, loadWindowFrame } from "./window-frame";

void (async () => {
  try {
    const frame = loadWindowFrame();
    if (frame) {
      await applyWindowFrame(frame);
    }
    await initShell();
  } catch (err) {
    console.error("[host] shell failed to start", err);
    const app = document.getElementById("app");
    if (app) {
      app.textContent = `Shell error: ${err}`;
    }
  } finally {
    try {
      const win = getCurrentWindow();
      if (!(await win.isVisible())) {
        await win.show();
      }
    } catch (err) {
      console.warn("[host] failed to show window:", err);
    }
  }
})();
