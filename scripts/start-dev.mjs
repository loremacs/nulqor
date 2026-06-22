#!/usr/bin/env node
/**
 * Cross-platform dev startup: best-effort stale process cleanup, then `tauri dev`.
 * Used by `npm start` — must work on Windows, macOS, and Linux.
 */
import { spawn, execSync } from "node:child_process";
import { platform } from "node:os";

function killStaleDevProcesses() {
  const os = platform();
  try {
    if (os === "darwin") {
      execSync("lsof -ti :1420 | xargs kill -9 2>/dev/null || true", {
        stdio: "ignore",
        shell: true,
      });
      execSync("pkill -x nulqor 2>/dev/null || true", {
        stdio: "ignore",
        shell: true,
      });
    } else if (os === "win32") {
      execSync(
        'powershell -NoProfile -Command "Get-NetTCPConnection -LocalPort 1420 -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.OwningProcess -Force -ErrorAction SilentlyContinue }"',
        { stdio: "ignore" },
      );
    } else if (os === "linux") {
      execSync("fuser -k 1420/tcp 2>/dev/null || true", {
        stdio: "ignore",
        shell: true,
      });
      execSync("pkill -x nulqor 2>/dev/null || true", {
        stdio: "ignore",
        shell: true,
      });
    }
  } catch {
    // Best-effort only — never block startup.
  }
}

killStaleDevProcesses();

const child = spawn("npx", ["tauri", "dev"], {
  stdio: "inherit",
  shell: true,
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
