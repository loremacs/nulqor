import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  applyMenuLayout,
  clampTileToDesk,
  lockTilePixels,
  minTileColSpan,
  minTileRowSpan,
  nearestMenuDock,
  pointerToGridCell,
  snapTileFromPointer,
  tileDisplayRect,
  tileFromWindowRect,
  tileSnapRect,
  tileWindowRect,
  updateGridGeometry,
  type GridMetrics,
} from "./grid";
import {
  applyProfileToGrid,
  applyProfileToSplit,
  captureGridProfile,
  captureSplitProfile,
  createSplitFromPreset,
  emptyProfileSlotName,
  MAX_CANVAS_PROFILES,
  normalizeProfileSlots,
  upsertProfile,
  type CanvasMode,
  type CanvasProfile,
} from "./canvas-profiles";
import {
  syncGridLayoutsFromSplitTree,
  syncSplitTreeFromGridLayouts,
  syncSubGridSettingsFromShell,
  syncGlobalPanelLayoutsFromSplitTree,
  dedupeOpenPanelIds,
  dedupePanelAssignmentsInTree,
  allPanelIdsInTree,
  ensureOpenPanelsInTree,
} from "./split-layout";
import {
  renderSplitLayout,
  removePanelFromTree,
  syncSplitTreeFromDom,
} from "./split-render";
import type { BuiltInPreset, SplitCanvasState } from "./split-layout";
import { BUILT_IN_PRESETS, defaultSplitState } from "./split-layout";
import { mountAlwaysOnTopKeeper } from "./always-on-top-keeper";
import { mountClickThrough, type ClickThroughHandle } from "./click-through";
import { mountMenuDockPreview } from "./menu-dock-preview";
import { promptSaveLayout } from "./save-layout-dialog";
import { isPanelResizable, mountPanel, registeredPanelIds } from "./panels";
import {
  CELL_PIXELS_MAX,
  CELL_STEP_MAX,
  clampCellPixels,
  clampCellStep,
  DEFAULT_SHELL,
  STORAGE_KEY,
  STORAGE_KEY_LEGACY,
  PANEL_MIN_HEIGHT_PX,
  PANEL_MIN_WIDTH_PX,
  type CanvasConfig,
  type MenuDock,
  type PersistedShellState,
  type ShellConfig,
  type TileLayout,
  type WindowFrameState,
} from "./types";
import {
  captureWindowFrame,
  DEFAULT_WINDOWED_FRAME,
  primeNativeBackgroundForMode,
  sanitizeWindowFrame,
  syncWindowFrameToDisk,
} from "./window-frame";
import { mountWindowResize } from "./window-resize";
import { mountWindowChrome } from "./window-chrome";
import type { WindowMode } from "./window-chrome";
import { isMacOS } from "./platform";

const DEFAULT_TILE_COLS = 4;
const DEFAULT_TILE_ROWS = 3;

type ShellToggleKey = "click_through" | "always_on_top";

const SHELL_TOGGLE_KEYS: ShellToggleKey[] = ["click_through", "always_on_top"];

/** Solid desk fill when not in fullscreen overlay mode. */
const WINDOWED_SHELL_BG = "#121216";

type LayoutToggleKey = "snap_enabled" | "show_grid" | "sash_snap_enabled";

function defaultTile(
  id: string,
  index: number,
  metrics: GridMetrics,
): TileLayout {
  const colSpan = DEFAULT_TILE_COLS;
  const rowSpan = DEFAULT_TILE_ROWS;
  // Step by colSpan so panels don't overlap (stepping by 1 put all panels in the
  // top-left since a 4-wide panel at col=1 and another at col=2 fully overlap).
  const slotsPerRow = Math.max(1, Math.floor(metrics.cols / colSpan));
  const col = 1 + (index % slotsPerRow) * colSpan;
  const row = 1 + Math.floor(index / slotsPerRow) * rowSpan;
  return { id, col, row, colSpan, rowSpan };
}

function loadPersisted(): PersistedShellState | null {
  try {
    let raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) raw = localStorage.getItem(STORAGE_KEY_LEGACY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as PersistedShellState & {
      tiles?: TileLayout[];
      shell?: ShellConfig & { grid_size?: number; grid_cols?: number };
    };
    if (parsed.shell) {
      parsed.shell.cell_pixels = clampCellPixels(
        Number(parsed.shell.cell_pixels) || DEFAULT_SHELL.cell_pixels,
      );
      parsed.shell.cell_step = clampCellStep(
        Number(parsed.shell.cell_step) || DEFAULT_SHELL.cell_step,
      );
    }
    if (!parsed.panelLayouts && parsed.tiles) {
      parsed.panelLayouts = Object.fromEntries(
        parsed.tiles.map((t) => [t.id, t]),
      );
      parsed.openPanelIds = parsed.tiles.map((t) => t.id);
    }
    if (!parsed.canvasMode) parsed.canvasMode = "grid";
    parsed.canvasProfiles = normalizeProfileSlots(parsed.canvasProfiles);
    if (parsed.activeProfileId === undefined) parsed.activeProfileId = null;
    // Never restore edit mode — it suspends click-through until "Done".
    parsed.layoutEditing = false;
    if (parsed.canvasMode === "split" && !parsed.split) {
      parsed.split = defaultSplitState("two-columns");
    }
    if (parsed.windowFrame) {
      parsed.windowFrame = sanitizeWindowFrame(parsed.windowFrame) ?? undefined;
    }
    return parsed;
  } catch {
    return null;
  }
}

function savePersisted(state: PersistedShellState): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
}

type BoundedNumericInputOptions = {
  input: HTMLInputElement;
  min: number;
  max: number;
  getCurrent: () => number;
  arrowStep: () => number;
  onCommit: (value: number) => void;
};

function wireBoundedNumericInput(opts: BoundedNumericInputOptions): () => void {
  const { input, min, max, getCurrent, arrowStep, onCommit } = opts;

  const clamp = (value: number): number => {
    const n = Math.round(value);
    if (!Number.isFinite(n)) return getCurrent();
    return Math.min(max, Math.max(min, n));
  };

  const commit = (): void => {
    const digits = input.value.replace(/\D/g, "");
    const next = clamp(digits === "" ? getCurrent() : Number(digits));
    input.value = String(next);
    onCommit(next);
  };

  let spinBase: number | null = null;

  input.addEventListener("mousedown", (event) => {
    const rect = input.getBoundingClientRect();
    if (event.clientX - rect.left > rect.width - 20) {
      const digits = input.value.replace(/\D/g, "");
      spinBase = clamp(digits === "" ? getCurrent() : Number(digits));
    }
  });

  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      commit();
      input.blur();
      return;
    }
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault();
      const digits = input.value.replace(/\D/g, "");
      const base = clamp(digits === "" ? getCurrent() : Number(digits));
      const step = arrowStep();
      const delta = event.key === "ArrowUp" ? step : -step;
      const next = clamp(base + delta);
      input.value = String(next);
      onCommit(next);
      return;
    }
    if (
      event.key === "e" ||
      event.key === "E" ||
      event.key === "+" ||
      event.key === "-" ||
      event.key === "." ||
      event.key === ","
    ) {
      event.preventDefault();
      return;
    }
    const allowed = [
      "Backspace",
      "Delete",
      "Tab",
      "Escape",
      "Enter",
      "ArrowLeft",
      "ArrowRight",
      "Home",
      "End",
    ];
    if (allowed.includes(event.key) || event.ctrlKey || event.metaKey) return;
    if (/^\d$/.test(event.key)) return;
    event.preventDefault();
  });

  input.addEventListener("input", () => {
    if (spinBase !== null) {
      const curr = Number(input.value);
      const step = arrowStep();
      if (Number.isFinite(curr) && Math.abs(curr - spinBase) === 1) {
        const next = clamp(spinBase + (curr > spinBase ? step : -step));
        input.value = String(next);
        onCommit(next);
        spinBase = null;
        return;
      }
      spinBase = null;
    }

    const digits = input.value.replace(/\D/g, "");
    if (digits !== input.value) {
      input.value = digits;
    }
    if (digits !== "" && Number(digits) > max) {
      input.value = String(max);
    }
  });

  input.addEventListener("paste", (event) => {
    event.preventDefault();
    const text = event.clipboardData?.getData("text").replace(/\D/g, "") ?? "";
    if (text === "") return;
    input.value = String(clamp(Number(text)));
  });

  input.addEventListener("blur", () => {
    spinBase = null;
    commit();
  });

  return () => {
    input.value = String(clamp(getCurrent()));
  };
}

async function fetchCanvasConfig(): Promise<CanvasConfig> {
  return invoke<CanvasConfig>("core_invoke", {
    id: "canvas:config@1",
    input: {},
  });
}

function applyShellCss(root: HTMLElement, shell: ShellConfig): void {
  root.classList.toggle("show-grid", shell.show_grid);
  root.classList.toggle("snap-enabled", shell.snap_enabled);
}

function setDropdownOpen(panel: HTMLElement, open: boolean): void {
  panel.hidden = !open;
  panel.classList.toggle("is-open", open);
}

function syncMenuCheckRow(row: HTMLElement, checked: boolean): void {
  row.setAttribute("aria-checked", String(checked));
  row.classList.toggle("is-checked", checked);
  const check = row.querySelector<HTMLElement>(".menu-dropdown-check");
  if (check) check.textContent = checked ? "✓" : "";
}

function setWindowModePresentation(mode: WindowMode): void {
  document.documentElement.dataset.nulqorWindowMode = mode;
  const startupHidden = document.documentElement.classList.contains(
    "nulqor-startup-hidden",
  );
  const bg =
    mode === "windowed" && !startupHidden ? WINDOWED_SHELL_BG : "transparent";
  document.documentElement.style.backgroundColor = bg;
  document.body.style.backgroundColor = bg;
  const app = document.getElementById("app");
  if (app) app.style.backgroundColor = bg;
}

function clampTile(tile: TileLayout, metrics: GridMetrics): TileLayout {
  return clampTileToDesk(tile, metrics);
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function clearTilePositionStyle(el: HTMLElement): void {
  el.style.gridColumn = "";
  el.style.gridRow = "";
  el.style.position = "";
  el.style.left = "";
  el.style.top = "";
  el.style.width = "";
  el.style.height = "";
  el.classList.remove("panel-tile-free", "panel-tile-snap");
}

function applyTileToElement(
  el: HTMLElement,
  tile: TileLayout,
  shell: ShellConfig,
  metrics: GridMetrics,
): void {
  clearTilePositionStyle(el);
  const rect = tileDisplayRect(tile, metrics, shell.snap_enabled);
  const isFree =
    !shell.snap_enabled && tile.freeX !== undefined && tile.freeY !== undefined;
  el.classList.add(isFree ? "panel-tile-free" : "panel-tile-snap");
  el.style.position = "absolute";
  el.style.left = `${rect.left}px`;
  el.style.top = `${rect.top}px`;
  el.style.width = `${Math.max(rect.width, PANEL_MIN_WIDTH_PX)}px`;
  el.style.height = `${Math.max(rect.height, PANEL_MIN_HEIGHT_PX)}px`;
}

function trackPointerSession(
  onMove: (event: PointerEvent) => void,
  onEnd: (event: PointerEvent) => void,
  pointerId?: number,
  captureTarget?: Element | null,
): () => void {
  let ended = false;

  const cleanup = (): void => {
    document.removeEventListener("pointermove", move, true);
    document.removeEventListener("pointerup", end, true);
    document.removeEventListener("pointercancel", end, true);
    if (captureTarget && pointerId !== undefined) {
      captureTarget.removeEventListener("lostpointercapture", onLostCapture);
      try {
        captureTarget.releasePointerCapture(pointerId);
      } catch {
        /* already released */
      }
    }
  };

  const finish = (event: PointerEvent): void => {
    if (ended) return;
    if (pointerId !== undefined && event.pointerId !== pointerId) return;
    ended = true;
    cleanup();
    onEnd(event);
  };

  const move = (event: PointerEvent): void => {
    if (ended) return;
    if (pointerId !== undefined && event.pointerId !== pointerId) return;
    onMove(event);
  };

  const end = (event: PointerEvent): void => {
    finish(event);
  };

  const onLostCapture = (event: Event): void => {
    finish(event as PointerEvent);
  };

  if (captureTarget && pointerId !== undefined) {
    try {
      captureTarget.setPointerCapture(pointerId);
      captureTarget.addEventListener("lostpointercapture", onLostCapture);
    } catch (err) {
      console.warn("[host] setPointerCapture failed:", err);
    }
  }

  document.addEventListener("pointermove", move, true);
  document.addEventListener("pointerup", end, true);
  document.addEventListener("pointercancel", end, true);

  return cleanup;
}

async function finishGridDragSession(
  clickThrough: ClickThroughHandle,
  resumeClickThrough: () => void,
): Promise<void> {
  resumeClickThrough();
  await clickThrough.flush();
  if (isMacOS()) {
    clickThrough.deferPassThrough(250);
  }
  clickThrough.refresh();
}

export type ShellHandle = {
  resyncWindowMode: () => Promise<void>;
  dispose: () => void;
};

export async function initShell(): Promise<ShellHandle> {
  const app = document.getElementById("app");
  if (!app) throw new Error("#app not found");

  if (isMacOS()) {
    document.documentElement.classList.add("platform-macos");
    // macOS click-to-activate: only refocus when the webview does not already have
    // focus. Calling setFocus on every pointerdown fights drag sessions and WKWebView.
    document.addEventListener(
      "pointerdown",
      () => {
        if (!document.hasFocus()) {
          void getCurrentWindow().setFocus();
        }
      },
      { capture: true, passive: true },
    );
  }

  const canvasConfig = await fetchCanvasConfig();
  const persisted = loadPersisted();

  let shell: ShellConfig = { ...DEFAULT_SHELL, ...canvasConfig.shell };
  if (persisted?.shell) shell = { ...shell, ...persisted.shell };
  shell.cell_pixels = clampCellPixels(shell.cell_pixels);
  shell.cell_step = clampCellStep(shell.cell_step);

  const panelLayouts: Record<string, TileLayout> = {
    ...persisted?.panelLayouts,
  };
  let openPanelIds: string[] = dedupeOpenPanelIds(
    persisted?.openPanelIds?.filter(
      (id) => panelLayouts[id] || canvasConfig.open_panels.includes(id),
    ) ?? [],
  );

  const startupPanels = canvasConfig.open_panels.filter((id) =>
    canvasConfig.panels.some((p) => p.id === id),
  );
  openPanelIds = dedupeOpenPanelIds([...openPanelIds, ...startupPanels]);

  if (openPanelIds.length === 0) {
    openPanelIds = dedupeOpenPanelIds([...canvasConfig.open_panels]);
  }

  let menuDock: MenuDock = isMacOS() ? "top" : (persisted?.menuDock ?? "top");
  const persistedFrame = sanitizeWindowFrame(persisted?.windowFrame ?? null);
  let windowMode: "fullscreen" | "windowed" =
    persistedFrame?.mode ?? "fullscreen";
  let windowFrame: WindowFrameState =
    persistedFrame ??
    (windowMode === "windowed"
      ? { ...DEFAULT_WINDOWED_FRAME }
      : { mode: "fullscreen", width: 1280, height: 720, x: 0, y: 0 });

  let canvasMode: CanvasMode = persisted?.canvasMode ?? "grid";
  let splitState: SplitCanvasState =
    persisted?.split ?? createSplitFromPreset("two-columns", openPanelIds);
  if (persisted?.split) {
    splitState = {
      ...splitState,
      tree: dedupePanelAssignmentsInTree(splitState.tree),
    };
    openPanelIds = dedupeOpenPanelIds([
      ...allPanelIdsInTree(splitState.tree),
      ...openPanelIds,
    ]);
  }
  if (canvasMode === "split") {
    splitState = {
      ...splitState,
      tree: ensureOpenPanelsInTree(splitState.tree, openPanelIds),
    };
  }
  let canvasProfiles = normalizeProfileSlots(persisted?.canvasProfiles);
  let activeProfileId: string | null = persisted?.activeProfileId ?? null;
  let layoutEditing = false;
  let renderGeneration = 0;
  let editClickThroughResume: (() => void) | null = null;

  app.innerHTML = `
    <div class="nulqor-shell" data-menu-dock="${menuDock}">
      <header class="menu-bar" data-interactive data-dock="${menuDock}">
        <div class="menu-bar-drag" title="Drag to move menu bar">
          <img class="menu-bar-icon" src="/extensions/host/ui/assets/nulqor-mark.svg" width="16" height="16" alt="" draggable="false" />
        </div>
        <nav class="menu-bar-menus" data-interactive>
          <div class="menu-group">
            <button type="button" class="menu-item" data-menu="settings" title="Settings" aria-label="Settings">
              <img class="menu-item-icon" src="/extensions/host/ui/assets/icon-settings.svg" width="16" height="16" alt="" draggable="false" />
              <span class="menu-item-label">Settings</span>
            </button>
            <div class="menu-dropdown" data-panel="settings" hidden role="menu">
              <button type="button" class="menu-dropdown-row menu-dropdown-row-check" data-setting="click_through" role="menuitemcheckbox" aria-checked="false">
                <span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Click Through Desktop <span class="menu-dropdown-hint">(fullscreen only)</span></span>
              </button>
              <button type="button" class="menu-dropdown-row menu-dropdown-row-check" data-setting="always_on_top" role="menuitemcheckbox" aria-checked="false">
                <span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Always on Top</span>
              </button>
              <button type="button" class="menu-dropdown-row" data-action="workbench-reset" title="Clears all Workbench enable/disable toggles for extensions, skills, rules, and agents. Does not change files or nulqor.toml.">
                <span class="menu-dropdown-gutter" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Reset Workbench toggles</span>
              </button>
              <div class="menu-dropdown-separator" role="separator"></div>
              <button type="button" class="menu-dropdown-row" data-action="exit" role="menuitem">
                <span class="menu-dropdown-gutter" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Exit</span>
              </button>
            </div>
          </div>
          <div class="menu-group">
            <button type="button" class="menu-item" data-menu="layout" title="Layout" aria-label="Layout">
              <img class="menu-item-icon" src="/extensions/host/ui/assets/icon-layout.svg" width="16" height="16" alt="" draggable="false" />
              <span class="menu-item-label">Layout</span>
            </button>
            <div class="menu-dropdown menu-dropdown-wide" data-panel="layout" hidden role="menu"></div>
          </div>
          <div class="menu-group">
            <button type="button" class="menu-item" data-menu="apps" title="Apps" aria-label="Apps">
              <img class="menu-item-icon" src="/extensions/host/ui/assets/icon-apps.svg" width="16" height="16" alt="" draggable="false" />
              <span class="menu-item-label">Apps</span>
            </button>
            <div class="menu-dropdown" data-panel="apps" hidden role="menu"></div>
          </div>
        </nav>
        <div class="menu-bar-spacer"></div>
        <div class="menu-window-controls">
          <button type="button" class="menu-window-btn" data-action="minimize" title="Minimize">—</button>
          <button type="button" class="menu-window-btn" data-action="restore" title="Restore down">❐</button>
          <button type="button" class="menu-window-btn menu-window-btn-close" data-action="close" title="Close">×</button>
        </div>
      </header>
      <div class="desktop-canvas desktop-grid"></div>
    </div>
  `;

  const shellRoot = app.querySelector<HTMLElement>(".nulqor-shell")!;
  const menuBar = app.querySelector<HTMLElement>(".menu-bar")!;
  const desktop = app.querySelector<HTMLElement>(".desktop-canvas")!;
  shellRoot.dataset.windowMode = windowMode;
  setWindowModePresentation(windowMode);
  const settingsPanel = app.querySelector<HTMLElement>(
    '[data-panel="settings"]',
  )!;
  const layoutPanel = app.querySelector<HTMLElement>('[data-panel="layout"]')!;
  const appsPanel = app.querySelector<HTMLElement>('[data-panel="apps"]')!;
  const layoutMenuBtn = app.querySelector<HTMLElement>('[data-menu="layout"]')!;

  let gridMetrics: GridMetrics;

  const refreshGrid = (): void => {
    gridMetrics = updateGridGeometry(shellRoot, desktop, shell);
  };

  let setMenuDock: (dock: MenuDock) => void;

  setMenuDock = (dock: MenuDock): void => {
    menuDock = dock;
    menuBar.dataset.dock = dock;
    shellRoot.dataset.menuDock = dock;
    applyMenuLayout(shellRoot, dock);
    refreshGrid();
  };

  applyShellCss(shellRoot, shell);
  setMenuDock(menuDock);

  const applyAlwaysOnTop = async (on: boolean): Promise<void> => {
    try {
      await getCurrentWindow().setAlwaysOnTop(on);
    } catch (err) {
      console.warn("[host] setAlwaysOnTop failed:", err);
    }
  };

  const topmostKeeper = mountAlwaysOnTopKeeper(
    () => shell.always_on_top && windowMode === "fullscreen",
    () => applyAlwaysOnTop(true),
  );

  const clickThrough = mountClickThrough(false, {
    onPassThrough: () => topmostKeeper.reassertNow(),
  });
  const windowResize = mountWindowResize(() => windowMode === "windowed");
  windowResize.setEnabled(windowMode === "windowed");
  const menuDockPreview = mountMenuDockPreview(shellRoot);
  let toastHideTimer: ReturnType<typeof setTimeout> | null = null;

  type ShellToastOptions = {
    durationMs?: number;
    tone?: "default" | "alert";
    placement?: "bottom" | "top";
  };

  const showShellToast = (
    message: string,
    options: ShellToastOptions = {},
  ): void => {
    const {
      durationMs = 3200,
      tone = "default",
      placement = "bottom",
    } = options;
    let toast = document.body.querySelector<HTMLElement>(".shell-toast");
    if (!toast) {
      toast = document.createElement("div");
      toast.className = "shell-toast";
      toast.setAttribute("role", tone === "alert" ? "alert" : "status");
      document.body.appendChild(toast);
    }
    toast.textContent = message;
    toast.className = "shell-toast";
    if (tone === "alert") toast.classList.add("shell-toast-alert");
    if (placement === "top") toast.classList.add("shell-toast-top");
    toast.classList.add("shell-toast-visible");
    if (toastHideTimer !== null) clearTimeout(toastHideTimer);
    toastHideTimer = window.setTimeout(() => {
      toast?.classList.remove("shell-toast-visible");
      toastHideTimer = null;
    }, durationMs);
  };

  const effectiveClickThrough = (): boolean =>
    windowMode === "fullscreen" && shell.click_through;

  const applyWindowModePolicy = (
    mode: WindowMode,
    previous: WindowMode,
    isRuntimeChange: boolean = false,
  ): void => {
    windowMode = mode;
    setWindowModePresentation(mode);
    windowResize.setEnabled(mode === "windowed");

    if (mode === "windowed") {
      clickThrough.setEnabled(false);
      clickThrough.forceClickable();
      void primeNativeBackgroundForMode("windowed");
      if (
        isRuntimeChange &&
        previous !== mode &&
        previous === "fullscreen" &&
        shell.click_through
      ) {
        showShellToast("Windowed mode — click-through disabled.", {
          tone: "alert",
          placement: "top",
          durationMs: 4000,
        });
      }
    } else {
      void primeNativeBackgroundForMode("fullscreen");
      clickThrough.setEnabled(shell.click_through);
    }

    topmostKeeper.sync(shell.always_on_top && mode === "fullscreen");

    syncSettingsInputs();
    syncClickThrough();

    if (isMacOS()) {
      void invoke("update_window_mode_label", { isFullscreen: mode === "fullscreen" });
    }
  };

  const syncClickThrough = (): void => {
    clickThrough.refresh();
  };

  /** Keep dragged panel above siblings; persist stack in openPanelIds / tiles. */
  const syncPanelStackOrder = (panelId: string): void => {
    openPanelIds = [...openPanelIds.filter((id) => id !== panelId), panelId];
    const tile = tiles.find((t) => t.id === panelId);
    if (tile) {
      tiles = [...tiles.filter((t) => t.id !== panelId), tile];
    }
  };

  const raiseGridPanel = (tileEl: HTMLElement, panelId: string): void => {
    tileEl.classList.add("panel-tile-dragging");
    desktop.appendChild(tileEl);
    syncPanelStackOrder(panelId);
  };

  const finishGridPanelDrag = (tileEl: HTMLElement): void => {
    tileEl.classList.remove("panel-tile-dragging");
  };

  void applyAlwaysOnTop(shell.always_on_top);
  topmostKeeper.sync(shell.always_on_top && windowMode === "fullscreen");

  let tiles: TileLayout[] = openPanelIds.map((id, i) =>
    clampTile(panelLayouts[id] ?? defaultTile(id, i, gridMetrics), gridMetrics),
  );
  tiles.forEach((t) => {
    panelLayouts[t.id] = t;
  });

  setMenuDock = (dock: MenuDock): void => {
    if (dock === menuDock) return;

    menuDockPreview.hide();

    const prevDesktop = desktop.getBoundingClientRect();
    const windowRects = tiles.map((t) =>
      tileWindowRect(t, gridMetrics, shell.snap_enabled, prevDesktop),
    );

    menuDock = dock;
    menuBar.dataset.dock = dock;
    shellRoot.dataset.menuDock = dock;
    applyMenuLayout(shellRoot, dock);
    refreshGrid();

    const nextDesktop = desktop.getBoundingClientRect();
    tiles = windowRects.map((rect, i) =>
      tileFromWindowRect(
        rect,
        nextDesktop,
        gridMetrics,
        tiles[i],
        shell.snap_enabled,
      ),
    );
    tiles.forEach((t) => {
      panelLayouts[t.id] = t;
    });
    void renderCanvas();
    syncClickThrough();
    menuBar.style.transform = "translateZ(0)";
    requestAnimationFrame(() => {
      menuBar.style.transform = "";
    });
    void shellRoot.offsetHeight;
  };

  const syncTilesFromLayouts = (): void => {
    tiles = openPanelIds.map((id, i) =>
      clampTile(
        panelLayouts[id] ?? defaultTile(id, i, gridMetrics),
        gridMetrics,
      ),
    );
  };

  const persist = (): void => {
    tiles.forEach((t) => {
      panelLayouts[t.id] = t;
    });
    savePersisted({
      menuDock,
      shell,
      canvasMode,
      panelLayouts,
      openPanelIds,
      split: splitState,
      canvasProfiles,
      activeProfileId,
      layoutEditing,
      windowFrame,
    });
  };

  const setLayoutEditing = (on: boolean): void => {
    layoutEditing = on;
    shellRoot.classList.toggle("canvas-editing", on);
    layoutMenuBtn.classList.toggle("is-editing", on);
    if (on) {
      if (!editClickThroughResume)
        editClickThroughResume = clickThrough.suspend();
    } else if (editClickThroughResume) {
      editClickThroughResume();
      editClickThroughResume = null;
    }
    persist();
    void renderCanvas();
    renderLayoutMenu();
    syncClickThrough();
  };

  const removeOrphanPanelTiles = (keepIds: Set<string>): void => {
    const seen = new Set<string>();
    for (const el of desktop.querySelectorAll<HTMLElement>(".panel-tile")) {
      const id = el.dataset.panelId;
      if (!id || !keepIds.has(id)) {
        el.remove();
        continue;
      }
      if (seen.has(id)) {
        el.remove();
        continue;
      }
      seen.add(id);
    }
  };

  const clearSplitArtifacts = (): void => {
    if (desktop.querySelector(".split-root, .split-container, .split-slot")) {
      desktop.innerHTML = "";
    }
  };

  const renderCanvas = async (): Promise<void> => {
    const gen = ++renderGeneration;
    if (canvasMode === "split") {
      await renderSplitLayout({
        desktop,
        shellRoot,
        shell,
        split: splitState,
        getTree: () => splitState.tree,
        layoutEditing,
        allowSlotDrag: true,
        panelLayouts,
        onTreeChange: (tree, preset) => {
          splitState = { preset, tree: dedupePanelAssignmentsInTree(tree) };
          persist();
          void renderCanvas();
        },
        onPersistSplit: (tree) => {
          const synced = syncSplitTreeFromDom(desktop, tree);
          splitState = {
            ...splitState,
            tree: dedupePanelAssignmentsInTree(synced),
          };
          persist();
        },
        onClosePanel: (panelId) => {
          panelLayouts[panelId] =
            tiles.find((t) => t.id === panelId) ?? panelLayouts[panelId];
          openPanelIds = openPanelIds.filter((id) => id !== panelId);
          splitState = {
            ...splitState,
            tree: removePanelFromTree(splitState.tree, panelId),
          };
          persist();
          void renderCanvas();
          renderAppsMenu();
          renderLayoutMenu();
        },
        suspendClickThrough: () => clickThrough.suspend(),
      });
      if (gen !== renderGeneration) return;
      syncClickThrough();
      return;
    }

    if (gen !== renderGeneration) return;
    clearSplitArtifacts();
    desktop.className = "desktop-canvas desktop-grid";
    if (layoutEditing) desktop.classList.add("canvas-editing");
    await renderTiles();
    if (gen !== renderGeneration) return;
  };

  const refreshWindowFrame = async (): Promise<void> => {
    windowFrame = await captureWindowFrame(windowFrame);
    persist();
  };

  const assignOpenPanelsToEmptyLeaves = (): void => {
    splitState = {
      ...splitState,
      tree: ensureOpenPanelsInTree(
        syncSplitTreeFromGridLayouts(
          splitState.tree,
          openPanelIds,
          panelLayouts,
          shell,
        ),
        openPanelIds,
      ),
    };
  };

  const applyCanvasProfile = (profile: CanvasProfile): void => {
    activeProfileId = profile.id;
    canvasMode = profile.mode;
    openPanelIds = dedupeOpenPanelIds(profile.openPanelIds);
    if (profile.mode === "grid" && profile.grid) {
      const applied = applyProfileToGrid(profile, shell);
      if (applied) {
        shell = applied.shell;
        for (const key of Object.keys(panelLayouts)) {
          delete panelLayouts[key];
        }
        Object.assign(panelLayouts, applied.panelLayouts);
        openPanelIds = dedupeOpenPanelIds(applied.openPanelIds);
        if (shell.snap_enabled) {
          for (const id of openPanelIds) {
            const layout = panelLayouts[id];
            if (layout) {
              panelLayouts[id] = {
                ...layout,
                pixelLock: undefined,
                freeX: undefined,
                freeY: undefined,
              };
            }
          }
        }
      }
    } else if (profile.mode === "split" && profile.split) {
      const applied = applyProfileToSplit(profile);
      if (applied) {
        if (applied.shell) {
          shell = { ...shell, ...applied.shell };
        }
        const tree = applied.tree;
        openPanelIds = dedupeOpenPanelIds(allPanelIdsInTree(tree));
        splitState = applied;
        syncGlobalPanelLayoutsFromSplitTree(tree, panelLayouts);
      }
    }
    applyShellCss(shellRoot, shell);
    refreshGrid();
    syncTilesFromLayouts();
    persist();
    void renderCanvas();
    renderAppsMenu();
    renderLayoutMenu();
    syncLayoutInputs();
  };

  const saveCurrentLayout = async (): Promise<void> => {
    const slotOptions = canvasProfiles.map((profile, index) => ({
      slotIndex: index,
      label: profile?.name ?? emptyProfileSlotName(index),
      occupied: profile !== null,
    }));
    const emptySlot = canvasProfiles.findIndex((profile) => profile === null);
    const defaultSlotIndex = emptySlot >= 0 ? emptySlot : 0;
    const defaultName =
      canvasProfiles[defaultSlotIndex]?.name ??
      emptyProfileSlotName(defaultSlotIndex);

    closeDropdowns();
    const resumeClickThrough = clickThrough.suspend();
    let result: { slotIndex: number; name: string } | null = null;
    try {
      result = await promptSaveLayout(
        shellRoot,
        slotOptions,
        defaultSlotIndex,
        defaultName,
      );
    } finally {
      resumeClickThrough();
    }
    if (!result) return;

    const { slotIndex, name } = result;
    const existing = canvasProfiles[slotIndex];
    if (canvasMode === "split") {
      const tree = dedupePanelAssignmentsInTree(
        syncSplitTreeFromDom(desktop, splitState.tree),
      );
      splitState = { ...splitState, tree };
      openPanelIds = dedupeOpenPanelIds(allPanelIdsInTree(tree));
    }
    const profile =
      canvasMode === "grid"
        ? captureGridProfile(name, shell, panelLayouts, openPanelIds)
        : captureSplitProfile(name, splitState, shell);
    if (existing) {
      profile.id = existing.id;
    }
    canvasProfiles = upsertProfile(canvasProfiles, slotIndex, profile);
    activeProfileId = profile.id;
    persist();
    renderLayoutMenu();
  };

  function menuSectionHeader(label: string): HTMLElement {
    const el = document.createElement("div");
    el.className = "menu-dropdown-section-header";
    el.setAttribute("role", "presentation");
    el.textContent = label;
    return el;
  }

  const renderLayoutMenu = (): void => {
    layoutPanel.innerHTML = "";

    layoutPanel.append(menuSectionHeader("Saved layouts"));

    for (let i = 0; i < MAX_CANVAS_PROFILES; i++) {
      const profile = canvasProfiles[i];
      const row = document.createElement("button");
      row.type = "button";
      row.className = "menu-dropdown-row menu-dropdown-row-check";
      row.role = "menuitemradio";
      row.dataset.profileSlot = String(i);
      if (profile) {
        row.dataset.profileId = profile.id;
        row.innerHTML =
          '<span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>' +
          `<span class="menu-dropdown-text">${escapeHtml(profile.name)}</span>`;
        syncMenuCheckRow(row, activeProfileId === profile.id);
      } else {
        row.disabled = true;
        row.classList.add("menu-dropdown-row-empty");
        row.innerHTML =
          '<span class="menu-dropdown-gutter" aria-hidden="true"></span>' +
          `<span class="menu-dropdown-text">${emptyProfileSlotName(i)} (empty)</span>`;
      }
      layoutPanel.append(row);
    }

    layoutPanel.append(menuSeparator());

    const modeGrid = document.createElement("button");
    modeGrid.type = "button";
    modeGrid.className = "menu-dropdown-row menu-dropdown-row-check";
    modeGrid.dataset.canvasMode = "grid";
    modeGrid.innerHTML =
      '<span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span><span class="menu-dropdown-text">Grid mode</span>';
    syncMenuCheckRow(modeGrid, canvasMode === "grid");
    layoutPanel.append(modeGrid);

    const modeSplit = document.createElement("button");
    modeSplit.type = "button";
    modeSplit.className = "menu-dropdown-row menu-dropdown-row-check";
    modeSplit.dataset.canvasMode = "split";
    modeSplit.innerHTML =
      '<span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span><span class="menu-dropdown-text">Layout mode</span>';
    syncMenuCheckRow(modeSplit, canvasMode === "split");
    layoutPanel.append(modeSplit);

    layoutPanel.append(menuSeparator());
    layoutPanel.append(menuSectionHeader("Presets"));
    for (const preset of BUILT_IN_PRESETS) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "menu-dropdown-row";
      row.dataset.splitPreset = preset.id;
      row.innerHTML =
        '<span class="menu-dropdown-gutter" aria-hidden="true"></span>' +
        `<span class="menu-dropdown-text">${escapeHtml(preset.label)}</span>`;
      layoutPanel.append(row);
    }

    layoutPanel.append(menuSeparator());

    const editRow = document.createElement("button");
    editRow.type = "button";
    editRow.className = "menu-dropdown-row";
    editRow.dataset.layoutAction = layoutEditing ? "done" : "edit";
    editRow.innerHTML =
      '<span class="menu-dropdown-gutter" aria-hidden="true"></span>' +
      `<span class="menu-dropdown-text">${layoutEditing ? "Done editing" : "Edit canvas…"}</span>`;
    layoutPanel.append(editRow);

    const saveRow = document.createElement("button");
    saveRow.type = "button";
    saveRow.className = "menu-dropdown-row";
    saveRow.dataset.layoutAction = "save";
    saveRow.innerHTML =
      '<span class="menu-dropdown-gutter" aria-hidden="true"></span><span class="menu-dropdown-text">Save current as…</span>';
    layoutPanel.append(saveRow);

    layoutPanel.append(menuSeparator());
    layoutPanel.append(menuSectionHeader("Grid"));

    const cellLabel = document.createElement("label");
    cellLabel.className =
      "menu-dropdown-row menu-dropdown-row-value menu-dropdown-row-sizes";
    cellLabel.innerHTML = `
        <span class="menu-dropdown-gutter" aria-hidden="true"></span>
        <span class="menu-dropdown-text">Cell Size</span>
        <div class="menu-dropdown-values">
          <input type="number" class="menu-dropdown-value" min="1" max="256" step="any" inputmode="numeric" data-setting="cell_pixels" aria-label="Cell size" />
          <span class="menu-dropdown-inline-label">Step</span>
          <input type="number" class="menu-dropdown-value" min="1" max="256" step="any" inputmode="numeric" data-setting="cell_step" aria-label="Cell size step" />
        </div>`;
    layoutPanel.append(cellLabel);

    for (const key of ["snap_enabled", "show_grid"] as LayoutToggleKey[]) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "menu-dropdown-row menu-dropdown-row-check";
      row.dataset.setting = key;
      row.role = "menuitemcheckbox";
      row.innerHTML =
        '<span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>' +
        `<span class="menu-dropdown-text">${key === "snap_enabled" ? "Snap to Grid" : "Show Grid Lines"}</span>`;
      syncMenuCheckRow(row, shell[key]);
      layoutPanel.append(row);
    }

    layoutPanel.append(menuSeparator());
    layoutPanel.append(menuSectionHeader("Split layout"));

    const sashSnapRow = document.createElement("button");
    sashSnapRow.type = "button";
    sashSnapRow.className = "menu-dropdown-row menu-dropdown-row-check";
    sashSnapRow.dataset.setting = "sash_snap_enabled";
    sashSnapRow.role = "menuitemcheckbox";
    sashSnapRow.innerHTML =
      '<span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>' +
      '<span class="menu-dropdown-text">Snap Layout Lines</span>';
    syncMenuCheckRow(sashSnapRow, shell.sash_snap_enabled);
    layoutPanel.append(sashSnapRow);

    wireLayoutMenuInputs();
  };

  function menuSeparator(): HTMLElement {
    const el = document.createElement("div");
    el.className = "menu-dropdown-separator";
    el.setAttribute("role", "separator");
    return el;
  }

  const wireLayoutMenuInputs = (): void => {
    const cellPixelsInput = layoutPanel.querySelector<HTMLInputElement>(
      '[data-setting="cell_pixels"]',
    );
    const cellStepInput = layoutPanel.querySelector<HTMLInputElement>(
      '[data-setting="cell_step"]',
    );
    if (!cellPixelsInput || !cellStepInput) return;

    const syncCellPixels = wireBoundedNumericInput({
      input: cellPixelsInput,
      min: 1,
      max: CELL_PIXELS_MAX,
      getCurrent: () => shell.cell_pixels,
      arrowStep: () => clampCellStep(shell.cell_step),
      onCommit: (next) => {
        cellPixelsInput.value = String(next);
        if (next !== shell.cell_pixels)
          applyShellSetting("cell_pixels", String(next));
      },
    });

    const syncCellStep = wireBoundedNumericInput({
      input: cellStepInput,
      min: 1,
      max: CELL_STEP_MAX,
      getCurrent: () => shell.cell_step,
      arrowStep: () => 1,
      onCommit: (next) => {
        cellStepInput.value = String(next);
        if (next !== shell.cell_step) {
          shell.cell_step = next;
          persist();
          syncLayoutInputs();
        }
      },
    });

    syncCellPixels();
    syncCellStep();
  };

  const syncLayoutInputs = (): void => {
    renderLayoutMenu();
  };

  const renderAppsMenu = (): void => {
    appsPanel.innerHTML = "";
    if (canvasConfig.panels.length === 0) {
      const empty = document.createElement("div");
      empty.className = "menu-dropdown-row menu-dropdown-row-disabled";
      empty.innerHTML =
        '<span class="menu-dropdown-gutter" aria-hidden="true"></span><span class="menu-dropdown-text">No Panel extensions enabled.</span>';
      appsPanel.append(empty);
      return;
    }

    for (const panel of canvasConfig.panels) {
      const open = openPanelIds.includes(panel.id);
      const hasLoader = registeredPanelIds().includes(panel.id);
      const row = document.createElement("button");
      row.type = "button";
      row.className = "menu-dropdown-row menu-dropdown-row-check";
      row.role = "menuitemcheckbox";
      row.dataset.panelId = panel.id;
      row.disabled = !hasLoader;
      row.innerHTML =
        '<span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>' +
        `<span class="menu-dropdown-text">${panel.id}${hasLoader ? "" : " (restart only)"}</span>`;
      syncMenuCheckRow(row, open);
      appsPanel.append(row);
    }
  };

  const removeClosedTiles = (): void => {
    for (const el of desktop.querySelectorAll<HTMLElement>(".panel-tile")) {
      if (!openPanelIds.includes(el.dataset.panelId!)) el.remove();
    }
  };

  const renderTiles = async (): Promise<void> => {
    refreshGrid();
    removeClosedTiles();
    removeOrphanPanelTiles(new Set(openPanelIds));
    for (const tile of tiles) {
      let el = desktop.querySelector<HTMLElement>(
        `:scope > .panel-tile[data-panel-id="${tile.id}"]`,
      );
      if (!el) {
        for (const orphan of desktop.querySelectorAll<HTMLElement>(
          `[data-panel-id="${tile.id}"]`,
        )) {
          orphan.remove();
        }
        el = document.createElement("article");
        el.className = "panel-tile";
        el.dataset.panelId = tile.id;
        el.dataset.interactive = "true";

        const header = document.createElement("header");
        header.className = "panel-tile-header";
        header.dataset.interactive = "true";
        header.innerHTML = `<span>${tile.id}</span><button type="button" data-close="${tile.id}" title="Close">×</button>`;

        const body = document.createElement("div");
        body.className = "panel-tile-body";
        body.dataset.interactive = "true";

        el.append(header, body);

        if (isPanelResizable(tile.id)) {
          const handle = document.createElement("div");
          handle.className = "panel-resize-handle";
          handle.dataset.interactive = "true";
          handle.title = "Resize";
          el.append(handle);
        }

        applyTileToElement(el, tile, shell, gridMetrics);
        desktop.append(el);
        await mountPanel(tile.id, body);
      }
      applyTileToElement(el, tile, shell, gridMetrics);
    }
    persist();
    syncClickThrough();
  };

  const syncSettingsInputs = (): void => {
    for (const key of SHELL_TOGGLE_KEYS) {
      const row = settingsPanel.querySelector<HTMLElement>(
        `[data-setting="${key}"]`,
      )!;
      if (key === "click_through") {
        const disabled = windowMode === "windowed";
        row.classList.toggle("menu-dropdown-row-disabled", disabled);
        row.setAttribute("aria-disabled", String(disabled));
        syncMenuCheckRow(row, effectiveClickThrough());
      } else {
        syncMenuCheckRow(row, shell[key]);
      }
    }
  };

  const applyWindowPref = (key: "click_through" | "always_on_top"): void => {
    if (key === "click_through" && windowMode === "windowed") {
      showShellToast("Click-through is only available in fullscreen.", {
        tone: "alert",
        placement: "top",
      });
      return;
    }
    shell[key] = !shell[key];
    syncSettingsInputs();
    if (key === "click_through") {
      clickThrough.setEnabled(effectiveClickThrough());
      syncClickThrough();
    } else {
      void applyAlwaysOnTop(shell.always_on_top);
      topmostKeeper.sync(shell.always_on_top && windowMode === "fullscreen");
    }
    persist();
    if (isMacOS()) {
      void invoke("update_menu_check", { id: `settings:${key}`, checked: key === "click_through" ? effectiveClickThrough() : shell[key] });
    }
  };

  const applyShellSetting = (
    key: "cell_pixels" | "snap_enabled" | "show_grid" | "sash_snap_enabled",
    rawValue?: string | boolean,
  ): void => {
    if (
      key === "snap_enabled" ||
      key === "show_grid" ||
      key === "sash_snap_enabled"
    ) {
      shell[key] = typeof rawValue === "boolean" ? rawValue : !shell[key];
    } else {
      shell.cell_pixels = clampCellPixels(Number(rawValue));
    }
    applyShellCss(shellRoot, shell);

    const prevMetrics = gridMetrics;

    if (key === "cell_pixels") {
      const splitManagedIds = new Set(allPanelIdsInTree(splitState.tree));
      const origin = desktop.getBoundingClientRect();
      tiles = tiles.map((t) => {
        if (canvasMode === "split" && splitManagedIds.has(t.id)) return t;
        const el = desktop.querySelector<HTMLElement>(
          `:scope > .panel-tile[data-panel-id="${t.id}"]`,
        );
        if (el) {
          const r = el.getBoundingClientRect();
          return {
            ...t,
            pixelLock: {
              left: r.left - origin.left,
              top: r.top - origin.top,
              width: r.width,
              height: r.height,
            },
            freeX: undefined,
            freeY: undefined,
          };
        }
        return lockTilePixels(t, prevMetrics, shell.snap_enabled);
      });
    }

    refreshGrid();

    if (key === "snap_enabled") {
      if (shell.snap_enabled) {
        tiles = tiles.map((t) => {
          if (t.freeX !== undefined && t.freeY !== undefined) {
            const origin = desktop.getBoundingClientRect();
            const cell = pointerToGridCell(
              origin.left + t.freeX,
              origin.top + t.freeY,
              desktop,
              gridMetrics,
            );
            return clampTile(
              {
                ...t,
                col: cell.col,
                row: cell.row,
                freeX: undefined,
                freeY: undefined,
              },
              gridMetrics,
            );
          }
          return clampTile(
            { ...t, freeX: undefined, freeY: undefined },
            gridMetrics,
          );
        });
      } else {
        tiles = tiles.map((t) => {
          if (t.freeX !== undefined && t.freeY !== undefined)
            return clampTile(t, gridMetrics);
          const rect = tileSnapRect(t, gridMetrics);
          return clampTile(
            { ...t, freeX: rect.left, freeY: rect.top },
            gridMetrics,
          );
        });
      }
    }

    tiles.forEach((t) => {
      panelLayouts[t.id] = t;
    });
    if (canvasMode === "split") {
      splitState = {
        ...splitState,
        tree: syncSubGridSettingsFromShell(
          splitState.tree,
          shell,
          key === "cell_pixels" ? prevMetrics.cellSize : undefined,
        ),
      };
    }
    persist();
    void renderCanvas();
  };

  syncSettingsInputs();
  renderAppsMenu();
  renderLayoutMenu();
  if (layoutEditing) {
    shellRoot.classList.add("canvas-editing");
    layoutMenuBtn.classList.add("is-editing");
    editClickThroughResume = clickThrough.suspend();
  }
  await renderCanvas();

  const closeDropdowns = (): void => {
    setDropdownOpen(settingsPanel, false);
    setDropdownOpen(layoutPanel, false);
    setDropdownOpen(appsPanel, false);
    syncClickThrough();
  };

  app
    .querySelector('[data-menu="settings"]')!
    .addEventListener("click", (e) => {
      e.stopPropagation();
      const willOpen = settingsPanel.hidden;
      closeDropdowns();
      setDropdownOpen(settingsPanel, willOpen);
      syncClickThrough();
    });

  layoutMenuBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    const willOpen = layoutPanel.hidden;
    closeDropdowns();
    if (willOpen) renderLayoutMenu();
    setDropdownOpen(layoutPanel, willOpen);
    syncClickThrough();
  });

  app.querySelector('[data-menu="apps"]')!.addEventListener("click", (e) => {
    e.stopPropagation();
    const willOpen = appsPanel.hidden;
    closeDropdowns();
    setDropdownOpen(appsPanel, willOpen);
    syncClickThrough();
  });

  const requestAppExit = (): void => {
    void getCurrentWindow().close();
  };

  settingsPanel.addEventListener("click", (event) => {
    const exitRow = (event.target as HTMLElement).closest<HTMLButtonElement>(
      "[data-action='exit']",
    );
    if (exitRow) {
      requestAppExit();
      return;
    }

    const actionRow = (event.target as HTMLElement).closest<HTMLButtonElement>(
      "[data-action='workbench-reset']",
    );
    if (actionRow) {
      void (async () => {
        try {
          await invoke("core_invoke", {
            id: "workbench:reset@1",
            input: {},
          });
          showShellToast("Workbench toggles reset — all extensions, skills, rules, and agents re-enabled.", {
            placement: "top",
          });
        } catch (err) {
          showShellToast(`Reset failed: ${err}`, {
            tone: "alert",
            placement: "top",
          });
        }
      })();
      return;
    }

    const row = (event.target as HTMLElement).closest<HTMLButtonElement>(
      ".menu-dropdown-row-check",
    );
    if (!row?.dataset.setting) return;
    const key = row.dataset.setting;
    if (
      key === "click_through" &&
      row.getAttribute("aria-disabled") === "true"
    ) {
      showShellToast("Click-through is only available in fullscreen.", {
        tone: "alert",
        placement: "top",
      });
      return;
    }
    if (row.getAttribute("aria-disabled") === "true") return;
    if (key === "click_through" || key === "always_on_top") {
      applyWindowPref(key);
    }
  });

  layoutPanel.addEventListener("click", (event) => {
    const target = event.target as HTMLElement;
    const row = target.closest<HTMLButtonElement>(".menu-dropdown-row");
    if (!row || row.disabled) return;

    const profileId = row.dataset.profileId;
    if (profileId) {
      const profile = canvasProfiles.find((p) => p?.id === profileId);
      if (profile) applyCanvasProfile(profile);
      return;
    }

    const mode = row.dataset.canvasMode as CanvasMode | undefined;
    if (mode && mode !== canvasMode) {
      canvasMode = mode;
      if (mode === "split") {
        assignOpenPanelsToEmptyLeaves();
      } else {
        const synced = syncGridLayoutsFromSplitTree(
          splitState.tree,
          openPanelIds,
          panelLayouts,
          (id, i) => defaultTile(id, i, gridMetrics),
        );
        openPanelIds = synced.openPanelIds;
        Object.assign(panelLayouts, synced.panelLayouts);
        syncTilesFromLayouts();
      }
      activeProfileId = null;
      persist();
      void renderCanvas();
      renderLayoutMenu();
      if (isMacOS()) {
        void invoke("update_menu_check", { id: "layout:grid",  checked: canvasMode === "grid" });
        void invoke("update_menu_check", { id: "layout:split", checked: canvasMode === "split" });
      }
      return;
    }

    const preset = row.dataset.splitPreset as BuiltInPreset | undefined;
    if (preset) {
      if (canvasMode !== "split") {
        canvasMode = "split";
      }
      splitState = createSplitFromPreset(preset, openPanelIds);
      activeProfileId = null;
      persist();
      void renderCanvas();
      renderLayoutMenu();
      if (isMacOS()) syncNativeMenuLayout();
      return;
    }

    const action = row.dataset.layoutAction;
    if (action === "edit") {
      setLayoutEditing(true);
      closeDropdowns();
      return;
    }
    if (action === "done") {
      setLayoutEditing(false);
      closeDropdowns();
      return;
    }
    if (action === "save") {
      void saveCurrentLayout();
      return;
    }

    const setting = row.dataset.setting;
    if (
      setting === "snap_enabled" ||
      setting === "show_grid" ||
      setting === "sash_snap_enabled"
    ) {
      applyShellSetting(setting);
      renderLayoutMenu();
      if (isMacOS() && (setting === "snap_enabled" || setting === "show_grid")) {
        const nativeId = setting === "snap_enabled" ? "layout:snap" : "layout:show_grid";
        void invoke("update_menu_check", { id: nativeId, checked: shell[setting] });
      }
    }
  });

  const restoreBtn = app.querySelector<HTMLButtonElement>(
    '[data-action="restore"]',
  )!;

  const windowChrome = mountWindowChrome({
    menuBar,
    shellRoot,
    restoreBtn,
    getWindowFrame: () => windowFrame,
    refreshWindowFrame,
    flushWindowFrame: () => {
      persist();
      void syncWindowFrameToDisk(windowFrame);
    },
    onLayoutChanged: () => {
      refreshGrid();
      if (canvasMode === "grid") {
        tiles = tiles.map((t) => clampTile(t, gridMetrics));
      }
      void renderCanvas();
      syncClickThrough();
    },
    onWindowModeChanged: (mode, previous) => {
      applyWindowModePolicy(mode, previous, true);
      persist();
    },
    // Side-docking is a Windows-only UX — macOS has a fixed top menu bar convention.
    ...(!isMacOS() && {
      onMenuDockDrag: (endEvent) => {
        menuDockPreview.hide();
        setMenuDock(nearestMenuDock(endEvent.clientX, endEvent.clientY));
        persist();
        syncClickThrough();
      },
      onMenuDockDragMove: (moveEvent) => {
        menuDockPreview.updateFromPointer(moveEvent.clientX, moveEvent.clientY);
      },
      onMenuDockDragEnd: () => {
        menuDockPreview.hide();
      },
    }),
    initialMode: windowMode,
  });

  await windowChrome.syncUi();
  applyWindowModePolicy(
    windowChrome.getWindowMode(),
    windowChrome.getWindowMode(),
  );

  menuBar.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    if (
      target.closest(
        ".menu-item, .menu-window-btn, .menu-dropdown, .menu-dropdown-row",
      )
    )
      return;
    const { pointerId } = event;
    const resume = clickThrough.suspend();
    const end = (e: PointerEvent): void => {
      if (e.pointerId !== pointerId) return;
      document.removeEventListener("pointerup", end);
      document.removeEventListener("pointercancel", end);
      resume();
      syncClickThrough();
    };
    document.addEventListener("pointerup", end);
    document.addEventListener("pointercancel", end);
  });

  window.addEventListener("resize", () => {
    refreshGrid();
    if (canvasMode === "grid") {
      tiles = tiles.map((t) => clampTile(t, gridMetrics));
    }
    void renderCanvas();
  });

  appsPanel.addEventListener("click", async (event) => {
    const row = (event.target as HTMLElement).closest<HTMLButtonElement>(
      ".menu-dropdown-row-check",
    );
    if (!row?.dataset.panelId || row.disabled) return;
    const panelId = row.dataset.panelId;
    const open = openPanelIds.includes(panelId);
    if (!open) {
      openPanelIds.push(panelId);
      if (!panelLayouts[panelId]) {
        panelLayouts[panelId] = defaultTile(
          panelId,
          openPanelIds.length - 1,
          gridMetrics,
        );
      }
      if (canvasMode === "split") {
        assignOpenPanelsToEmptyLeaves();
      }
    } else {
      panelLayouts[panelId] =
        tiles.find((t) => t.id === panelId) ?? panelLayouts[panelId];
      openPanelIds = openPanelIds.filter((id) => id !== panelId);
      if (canvasMode === "split") {
        splitState = {
          ...splitState,
          tree: removePanelFromTree(splitState.tree, panelId),
        };
      }
    }
    syncTilesFromLayouts();
    persist();
    await renderCanvas();
    renderAppsMenu();
    renderLayoutMenu();
    if (isMacOS()) {
      void invoke("update_menu_check", { id: `apps:${panelId}`, checked: !open });
    }
  });

  desktop.addEventListener("click", async (event) => {
    if (canvasMode !== "grid") return;
    const closeId = (event.target as HTMLElement).dataset.close;
    if (!closeId) return;
    panelLayouts[closeId] =
      tiles.find((t) => t.id === closeId) ?? panelLayouts[closeId];
    openPanelIds = openPanelIds.filter((id) => id !== closeId);
    syncTilesFromLayouts();
    await renderCanvas();
    renderAppsMenu();
    renderLayoutMenu();
    if (isMacOS()) {
      void invoke("update_menu_check", { id: `apps:${closeId}`, checked: false });
    }
  });

  let tileDrag: {
    id: string;
    el: HTMLElement;
    pointerOffsetX: number;
    pointerOffsetY: number;
  } | null = null;

  let tileResize: { id: string; el: HTMLElement } | null = null;
  let activeGridPointerCleanup: (() => void) | null = null;

  const endGridPointerSession = async (
    resumeClickThrough: () => void,
  ): Promise<void> => {
    activeGridPointerCleanup?.();
    activeGridPointerCleanup = null;
    await finishGridDragSession(clickThrough, resumeClickThrough);
  };

  const desktopOrigin = (): { left: number; top: number } => {
    const rect = desktop.getBoundingClientRect();
    return { left: rect.left, top: rect.top };
  };

  const positionTileFromPointer = (
    clientX: number,
    clientY: number,
  ): TileLayout | null => {
    if (!tileDrag) return null;
    const tile = tiles.find((t) => t.id === tileDrag!.id);
    if (!tile) return null;

    if (!shell.snap_enabled) {
      const origin = desktopOrigin();
      const freeX = clientX - tileDrag.pointerOffsetX - origin.left;
      const freeY = clientY - tileDrag.pointerOffsetY - origin.top;
      return { ...tile, freeX, freeY, pixelLock: undefined };
    }

    const topLeftX = clientX - tileDrag.pointerOffsetX;
    const topLeftY = clientY - tileDrag.pointerOffsetY;

    if (tile.pixelLock) {
      const cell = pointerToGridCell(topLeftX, topLeftY, desktop, gridMetrics);
      return {
        ...tile,
        col: cell.col,
        row: cell.row,
        pixelLock: {
          ...tile.pixelLock,
          left: (cell.col - 1) * gridMetrics.colStep,
          top: (cell.row - 1) * gridMetrics.rowStep,
        },
      };
    }

    return snapTileFromPointer(tile, topLeftX, topLeftY, desktop, gridMetrics);
  };

  const applyResize = (clientX: number, clientY: number): void => {
    if (!tileResize) return;
    const tile = tiles.find((t) => t.id === tileResize!.id);
    if (!tile) return;
    const endCell = pointerToGridCell(clientX, clientY, desktop, gridMetrics);
    const colSpan = Math.max(
      minTileColSpan(gridMetrics),
      endCell.col - tile.col + 1,
    );
    const rowSpan = Math.max(
      minTileRowSpan(gridMetrics),
      endCell.row - tile.row + 1,
    );
    const next = clampTile(
      { ...tile, colSpan, rowSpan, pixelLock: undefined },
      gridMetrics,
    );
    tiles = tiles.map((t) => (t.id === tileResize!.id ? next : t));
    applyTileToElement(tileResize.el, next, shell, gridMetrics);
    persist();
  };

  // Capture phase so click-through pass-through cannot swallow panel headers on
  // macOS before we suspend OS pass-through for the drag session.
  document.addEventListener("pointerdown", (event) => {
    if (canvasMode !== "grid") return;
    const target = event.target as HTMLElement;
    if (!target.closest(".desktop-canvas")) return;

    const handle = target.closest(".panel-resize-handle");
    if (handle) {
      const tileEl = handle.closest<HTMLElement>(".panel-tile");
      if (!tileEl) return;
      const id = tileEl.dataset.panelId!;
      if (!isPanelResizable(id)) return;
      activeGridPointerCleanup?.();
      tileResize = { id, el: tileEl };
      desktop.appendChild(tileEl);
      syncPanelStackOrder(id);
      event.preventDefault();
      event.stopPropagation();
      const resumeClickThrough = clickThrough.suspend();
      void clickThrough.flush();
      activeGridPointerCleanup = trackPointerSession(
        (moveEvent) => applyResize(moveEvent.clientX, moveEvent.clientY),
        () => {
          tileResize = null;
          persist();
          void endGridPointerSession(resumeClickThrough);
        },
        event.pointerId,
        handle,
      );
      return;
    }

    // Prefer the direct target's header; fall back to elementsFromPoint so that
    // a tile whose body overlaps another tile's header can still be dragged.
    let headerEl = target.closest<HTMLElement>(".panel-tile-header");
    if (!headerEl) {
      for (const el of document.elementsFromPoint(event.clientX, event.clientY)) {
        const h = (el as HTMLElement).closest<HTMLElement>(".panel-tile-header");
        if (h) { headerEl = h; break; }
      }
    }
    if (!headerEl) return;
    const tileEl = headerEl.closest<HTMLElement>(".panel-tile");
    if (!tileEl) return;
    const id = tileEl.dataset.panelId!;
    activeGridPointerCleanup?.();
    const tileRect = tileEl.getBoundingClientRect();
    tileDrag = {
      id,
      el: tileEl,
      pointerOffsetX: event.clientX - tileRect.left,
      pointerOffsetY: event.clientY - tileRect.top,
    };
    raiseGridPanel(tileEl, id);
    event.preventDefault();
    event.stopPropagation();
    const resumeClickThrough = clickThrough.suspend();
    void clickThrough.flush();
    activeGridPointerCleanup = trackPointerSession(
      (moveEvent) => {
        const next = positionTileFromPointer(
          moveEvent.clientX,
          moveEvent.clientY,
        );
        if (next) applyTileToElement(tileDrag!.el, next, shell, gridMetrics);
      },
      (endEvent) => {
        const dragged = tiles.find((t) => t.id === tileDrag!.id);
        if (!dragged) {
          finishGridPanelDrag(tileDrag!.el);
          tileDrag = null;
          void endGridPointerSession(resumeClickThrough);
          return;
        }
        const topLeftX = endEvent.clientX - tileDrag!.pointerOffsetX;
        const topLeftY = endEvent.clientY - tileDrag!.pointerOffsetY;
        let resolved =
          shell.snap_enabled && dragged.pixelLock
            ? snapTileFromPointer(
                dragged,
                topLeftX,
                topLeftY,
                desktop,
                gridMetrics,
              )
            : positionTileFromPointer(endEvent.clientX, endEvent.clientY);
        // In snap mode, reject positions that would overlap another tile. Snap
        // back to the original position so the tile remains independently draggable.
        if (resolved && shell.snap_enabled) {
          const dragId = tileDrag!.id;
          const overlaps = tiles.some((t) => {
            if (t.id === dragId) return false;
            return (
              resolved!.col < t.col + t.colSpan &&
              resolved!.col + resolved!.colSpan > t.col &&
              resolved!.row < t.row + t.rowSpan &&
              resolved!.row + resolved!.rowSpan > t.row
            );
          });
          if (overlaps) {
            applyTileToElement(tileDrag!.el, dragged, shell, gridMetrics);
            resolved = null;
          }
        }
        if (resolved) {
          tiles = tiles.map((t) => (t.id === tileDrag!.id ? resolved! : t));
          panelLayouts[resolved.id] = resolved;
        }
        finishGridPanelDrag(tileDrag!.el);
        tileDrag = null;
        persist();
        void endGridPointerSession(resumeClickThrough);
      },
      event.pointerId,
      headerEl,
    );
  }, true);

  document.addEventListener("click", (event) => {
    const target = event.target as Element;
    if (!target.closest(".menu-group")) closeDropdowns();
  });

  document.addEventListener("pointerdown", (event) => {
    const target = event.target as Element;
    if (!target.closest(".menu-group")) closeDropdowns();
  });

  // ── macOS native menu bar integration ───────────────────────────────────
  // Sync helpers push JS state into the native NSMenu check items after
  // any action (webview or native menu) changes local state.
  const syncNativeMenuSettings = (): void => {
    void invoke("update_menu_check", { id: "settings:click_through", checked: effectiveClickThrough() });
    void invoke("update_menu_check", { id: "settings:always_on_top", checked: shell.always_on_top });
  };

  const syncNativeMenuLayout = (): void => {
    void invoke("update_menu_check", { id: "layout:grid",      checked: canvasMode === "grid" });
    void invoke("update_menu_check", { id: "layout:split",     checked: canvasMode === "split" });
    void invoke("update_menu_check", { id: "layout:snap",      checked: shell.snap_enabled });
    void invoke("update_menu_check", { id: "layout:show_grid", checked: shell.show_grid });
  };

  const syncNativeMenuPanels = (): void => {
    for (const panel of canvasConfig.panels) {
      void invoke("update_menu_check", { id: `apps:${panel.id}`, checked: openPanelIds.includes(panel.id) });
    }
  };

  let unlistenMenuAction: (() => void) | null = null;

  if (isMacOS()) {
    unlistenMenuAction = await listen<string>("nulqor:menu-action", async (event) => {
      const id = event.payload;

      if (id === "settings:click_through") {
        applyWindowPref("click_through");
      } else if (id === "settings:always_on_top") {
        applyWindowPref("always_on_top");
      } else if (id === "settings:workbench_reset") {
        try {
          await invoke("core_invoke", { id: "workbench:reset@1", input: {} });
          showShellToast("Workbench toggles reset.", { placement: "top" });
        } catch (err) {
          showShellToast(`Reset failed: ${err}`, { tone: "alert", placement: "top" });
        }
      } else if (id === "settings:exit") {
        requestAppExit();
      } else if (id === "layout:grid") {
        if (canvasMode !== "grid") {
          const synced = syncGridLayoutsFromSplitTree(
            splitState.tree, openPanelIds, panelLayouts,
            (panelId, i) => defaultTile(panelId, i, gridMetrics),
          );
          openPanelIds = synced.openPanelIds;
          Object.assign(panelLayouts, synced.panelLayouts);
          canvasMode = "grid";
          syncTilesFromLayouts();
          activeProfileId = null;
          persist();
          void renderCanvas();
          renderLayoutMenu();
        }
        syncNativeMenuLayout();
      } else if (id === "layout:split") {
        if (canvasMode !== "split") {
          canvasMode = "split";
          assignOpenPanelsToEmptyLeaves();
          activeProfileId = null;
          persist();
          void renderCanvas();
          renderLayoutMenu();
        }
        syncNativeMenuLayout();
      } else if (id === "layout:snap") {
        applyShellSetting("snap_enabled");
        syncNativeMenuLayout();
      } else if (id === "layout:show_grid") {
        applyShellSetting("show_grid");
        syncNativeMenuLayout();
      } else if (id === "window:toggle_mode") {
        await windowChrome.toggleFullscreen();
      } else if (id.startsWith("apps:")) {
        const panelId = id.slice("apps:".length);
        const open = openPanelIds.includes(panelId);
        if (!open) {
          openPanelIds.push(panelId);
          if (!panelLayouts[panelId]) {
            panelLayouts[panelId] = defaultTile(panelId, openPanelIds.length - 1, gridMetrics);
          }
          if (canvasMode === "split") assignOpenPanelsToEmptyLeaves();
        } else {
          panelLayouts[panelId] = tiles.find((t) => t.id === panelId) ?? panelLayouts[panelId];
          openPanelIds = openPanelIds.filter((pid) => pid !== panelId);
          if (canvasMode === "split") {
            splitState = { ...splitState, tree: removePanelFromTree(splitState.tree, panelId) };
          }
        }
        syncTilesFromLayouts();
        persist();
        await renderCanvas();
        renderAppsMenu();
        renderLayoutMenu();
        syncNativeMenuPanels();
      }
    });

    // Prime the native menu with the current runtime state.
    syncNativeMenuSettings();
    syncNativeMenuLayout();
    syncNativeMenuPanels();
    void invoke("update_window_mode_label", { isFullscreen: windowMode === "fullscreen" });
  }

  return {
    resyncWindowMode: async () => {
      await windowChrome.syncUi();
      applyWindowModePolicy(
        windowChrome.getWindowMode(),
        windowChrome.getWindowMode(),
      );
    },
    dispose: () => {
      unlistenMenuAction?.();
      unlistenMenuAction = null;
    },
  };
}
