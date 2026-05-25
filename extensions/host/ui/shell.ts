import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  applyMenuLayout,
  clampTileToDesk,
  lockTilePixels,
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
import { mountClickThrough } from "./click-through";
import { mountMenuDockPreview } from "./menu-dock-preview";
import { isPanelResizable, mountPanel, registeredPanelIds } from "./panels";
import {
  CELL_PIXELS_MAX,
  CELL_STEP_MAX,
  clampCellPixels,
  clampCellStep,
  DEFAULT_SHELL,
  STORAGE_KEY,
  type CanvasConfig,
  type MenuDock,
  type PersistedShellState,
  type ShellConfig,
  type TileLayout,
  type WindowFrameState,
} from "./types";
import { captureWindowFrame, DEFAULT_WINDOWED_FRAME } from "./window-frame";
import { mountWindowChrome } from "./window-chrome";

const DEFAULT_TILE_COLS = 4;
const DEFAULT_TILE_ROWS = 3;
const MIN_TILE_COLS = 1;
const MIN_TILE_ROWS = 1;

type ShellToggleKey =
  | "snap_enabled"
  | "show_grid"
  | "click_through"
  | "always_on_top";

const SHELL_TOGGLE_KEYS: ShellToggleKey[] = [
  "snap_enabled",
  "show_grid",
  "click_through",
  "always_on_top",
];

function defaultTile(
  id: string,
  index: number,
  metrics: GridMetrics,
): TileLayout {
  const colSpan = DEFAULT_TILE_COLS;
  const rowSpan = DEFAULT_TILE_ROWS;
  const slotsPerRow = Math.max(1, metrics.cols - colSpan + 1);
  const col = 1 + (index % slotsPerRow);
  const row = 1 + Math.floor(index / slotsPerRow) * rowSpan;
  return { id, col, row, colSpan, rowSpan };
}

function loadPersisted(): PersistedShellState | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
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

function clampTile(tile: TileLayout, metrics: GridMetrics): TileLayout {
  return clampTileToDesk(tile, metrics);
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
  el.style.width = `${rect.width}px`;
  el.style.height = `${rect.height}px`;
}

function trackPointerSession(
  onMove: (event: PointerEvent) => void,
  onEnd: (event: PointerEvent) => void,
): void {
  const move = (event: PointerEvent): void => onMove(event);
  const end = (event: PointerEvent): void => {
    document.removeEventListener("pointermove", move);
    document.removeEventListener("pointerup", end);
    document.removeEventListener("pointercancel", end);
    onEnd(event);
  };
  document.addEventListener("pointermove", move);
  document.addEventListener("pointerup", end);
  document.addEventListener("pointercancel", end);
}

export async function initShell(): Promise<void> {
  const app = document.getElementById("app");
  if (!app) throw new Error("#app not found");

  const canvasConfig = await fetchCanvasConfig();
  const persisted = loadPersisted();

  let shell: ShellConfig = { ...DEFAULT_SHELL, ...canvasConfig.shell };
  if (persisted?.shell) shell = { ...shell, ...persisted.shell };
  shell.cell_pixels = clampCellPixels(shell.cell_pixels);
  shell.cell_step = clampCellStep(shell.cell_step);

  const panelLayouts: Record<string, TileLayout> = {
    ...persisted?.panelLayouts,
  };
  let openPanelIds: string[] =
    persisted?.openPanelIds?.filter(
      (id) => panelLayouts[id] || canvasConfig.open_panels.includes(id),
    ) ?? [];

  if (openPanelIds.length === 0) {
    openPanelIds = [...canvasConfig.open_panels];
  }

  let menuDock: MenuDock = persisted?.menuDock ?? "top";
  let windowMode: "fullscreen" | "windowed" =
    persisted?.windowFrame?.mode ?? "fullscreen";
  let windowFrame: WindowFrameState =
    persisted?.windowFrame ??
    (windowMode === "windowed"
      ? { ...DEFAULT_WINDOWED_FRAME }
      : { mode: "fullscreen", width: 1280, height: 720, x: 0, y: 0 });

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
              <label class="menu-dropdown-row menu-dropdown-row-value menu-dropdown-row-sizes">
                <span class="menu-dropdown-gutter" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Cell Size</span>
                <div class="menu-dropdown-values">
                  <input type="number" class="menu-dropdown-value" min="1" max="256" step="any" inputmode="numeric" data-setting="cell_pixels" aria-label="Cell size" />
                  <span class="menu-dropdown-inline-label">Step</span>
                  <input type="number" class="menu-dropdown-value" min="1" max="256" step="any" inputmode="numeric" data-setting="cell_step" aria-label="Cell size step" />
                </div>
              </label>
              <div class="menu-dropdown-separator" role="separator"></div>
              <button type="button" class="menu-dropdown-row menu-dropdown-row-check" data-setting="snap_enabled" role="menuitemcheckbox" aria-checked="false">
                <span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Snap to Grid</span>
              </button>
              <button type="button" class="menu-dropdown-row menu-dropdown-row-check" data-setting="show_grid" role="menuitemcheckbox" aria-checked="false">
                <span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Show Grid Lines</span>
              </button>
              <div class="menu-dropdown-separator" role="separator"></div>
              <button type="button" class="menu-dropdown-row menu-dropdown-row-check" data-setting="click_through" role="menuitemcheckbox" aria-checked="false">
                <span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Click Through Desktop</span>
              </button>
              <button type="button" class="menu-dropdown-row menu-dropdown-row-check" data-setting="always_on_top" role="menuitemcheckbox" aria-checked="false">
                <span class="menu-dropdown-gutter menu-dropdown-check" aria-hidden="true"></span>
                <span class="menu-dropdown-text">Always on Top</span>
              </button>
            </div>
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
      <div class="desktop-grid"></div>
    </div>
  `;

  const shellRoot = app.querySelector<HTMLElement>(".nulqor-shell")!;
  const menuBar = app.querySelector<HTMLElement>(".menu-bar")!;
  const desktop = app.querySelector<HTMLElement>(".desktop-grid")!;
  const settingsPanel = app.querySelector<HTMLElement>(
    '[data-panel="settings"]',
  )!;
  const appsPanel = app.querySelector<HTMLElement>('[data-panel="apps"]')!;

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

  const clickThrough = mountClickThrough(shell.click_through);
  const menuDockPreview = mountMenuDockPreview(shellRoot);
  const syncClickThrough = (): void => {
    clickThrough.refresh();
  };

  const applyAlwaysOnTop = async (on: boolean): Promise<void> => {
    try {
      await getCurrentWindow().setAlwaysOnTop(on);
    } catch (err) {
      console.warn("[host] setAlwaysOnTop failed:", err);
    }
  };

  void applyAlwaysOnTop(shell.always_on_top);

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
    void renderTiles();
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
    savePersisted({ menuDock, shell, panelLayouts, openPanelIds, windowFrame });
  };

  const refreshWindowFrame = async (): Promise<void> => {
    windowFrame = await captureWindowFrame();
    windowMode = windowFrame.mode;
    persist();
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
    removeClosedTiles();
    for (const tile of tiles) {
      let el = desktop.querySelector<HTMLElement>(
        `[data-panel-id="${tile.id}"]`,
      );
      if (!el) {
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

  const cellPixelsInput = settingsPanel.querySelector<HTMLInputElement>(
    '[data-setting="cell_pixels"]',
  )!;
  const cellStepInput = settingsPanel.querySelector<HTMLInputElement>(
    '[data-setting="cell_step"]',
  )!;

  const syncCellSizeInputs = (): void => {
    cellPixelsInput.value = String(clampCellPixels(shell.cell_pixels));
    cellStepInput.value = String(clampCellStep(shell.cell_step));
  };

  const syncSettingsInputs = (): void => {
    syncCellSizeInputs();
    for (const key of SHELL_TOGGLE_KEYS) {
      const row = settingsPanel.querySelector<HTMLElement>(
        `[data-setting="${key}"]`,
      )!;
      syncMenuCheckRow(row, shell[key]);
    }
  };

  const applyWindowPref = (key: "click_through" | "always_on_top"): void => {
    shell[key] = !shell[key];
    syncSettingsInputs();
    if (key === "click_through") {
      clickThrough.setEnabled(shell.click_through);
      syncClickThrough();
    } else {
      void applyAlwaysOnTop(shell.always_on_top);
    }
    persist();
  };

  const applyShellSetting = (
    key: "cell_pixels" | "snap_enabled" | "show_grid",
    rawValue?: string | boolean,
  ): void => {
    if (key === "snap_enabled" || key === "show_grid") {
      shell[key] = typeof rawValue === "boolean" ? rawValue : !shell[key];
    } else {
      shell.cell_pixels = clampCellPixels(Number(rawValue));
    }
    applyShellCss(shellRoot, shell);
    syncSettingsInputs();

    const prevMetrics = gridMetrics;
    refreshGrid();

    if (key === "cell_pixels") {
      tiles = tiles.map((t) =>
        lockTilePixels(t, prevMetrics, shell.snap_enabled),
      );
    } else if (shell.snap_enabled) {
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

    tiles.forEach((t) => {
      panelLayouts[t.id] = t;
    });
    void renderTiles();
  };

  wireBoundedNumericInput({
    input: cellPixelsInput,
    min: 1,
    max: CELL_PIXELS_MAX,
    getCurrent: () => shell.cell_pixels,
    arrowStep: () => clampCellStep(shell.cell_step),
    onCommit: (next) => {
      cellPixelsInput.value = String(next);
      if (next !== shell.cell_pixels) {
        applyShellSetting("cell_pixels", String(next));
      }
    },
  });

  wireBoundedNumericInput({
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
        syncCellSizeInputs();
      }
    },
  });

  syncSettingsInputs();
  renderAppsMenu();
  await renderTiles();

  const closeDropdowns = (): void => {
    setDropdownOpen(settingsPanel, false);
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

  app.querySelector('[data-menu="apps"]')!.addEventListener("click", (e) => {
    e.stopPropagation();
    const willOpen = appsPanel.hidden;
    closeDropdowns();
    setDropdownOpen(appsPanel, willOpen);
    syncClickThrough();
  });

  settingsPanel.addEventListener("change", (event) => {
    const target = event.target as HTMLInputElement;
    const key = target.dataset.setting;
    if (key !== "cell_pixels" && key !== "cell_step") return;
    target.blur();
  });

  settingsPanel.addEventListener("click", (event) => {
    const row = (event.target as HTMLElement).closest<HTMLButtonElement>(
      ".menu-dropdown-row-check",
    );
    if (!row?.dataset.setting) return;
    const key = row.dataset.setting;
    if (key === "click_through" || key === "always_on_top") {
      applyWindowPref(key);
      return;
    }
    if (key === "snap_enabled" || key === "show_grid") {
      applyShellSetting(key);
    }
  });

  const restoreBtn = app.querySelector<HTMLButtonElement>(
    '[data-action="restore"]',
  )!;

  mountWindowChrome({
    menuBar,
    shellRoot,
    restoreBtn,
    getWindowFrame: () => windowFrame,
    refreshWindowFrame,
    onLayoutChanged: () => {
      refreshGrid();
      tiles = tiles.map((t) => clampTile(t, gridMetrics));
      void renderTiles();
      syncClickThrough();
    },
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
    initialMode: windowMode,
  });

  menuBar.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    if (
      target.closest(
        ".menu-item, .menu-window-btn, .menu-dropdown, .menu-dropdown-row",
      )
    )
      return;
    const resume = clickThrough.suspend();
    const end = (): void => {
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
    tiles = tiles.map((t) => clampTile(t, gridMetrics));
    void renderTiles();
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
    } else {
      panelLayouts[panelId] =
        tiles.find((t) => t.id === panelId) ?? panelLayouts[panelId];
      openPanelIds = openPanelIds.filter((id) => id !== panelId);
    }
    syncTilesFromLayouts();
    await renderTiles();
    renderAppsMenu();
  });

  desktop.addEventListener("click", async (event) => {
    const closeId = (event.target as HTMLElement).dataset.close;
    if (!closeId) return;
    panelLayouts[closeId] =
      tiles.find((t) => t.id === closeId) ?? panelLayouts[closeId];
    openPanelIds = openPanelIds.filter((id) => id !== closeId);
    syncTilesFromLayouts();
    await renderTiles();
    renderAppsMenu();
  });

  let tileDrag: {
    id: string;
    el: HTMLElement;
    pointerOffsetX: number;
    pointerOffsetY: number;
  } | null = null;

  let tileResize: { id: string; el: HTMLElement } | null = null;

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
          left: (cell.col - 1) * gridMetrics.step,
          top: (cell.row - 1) * gridMetrics.step,
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
    const colSpan = Math.max(MIN_TILE_COLS, endCell.col - tile.col + 1);
    const rowSpan = Math.max(MIN_TILE_ROWS, endCell.row - tile.row + 1);
    const next = clampTile(
      { ...tile, colSpan, rowSpan, pixelLock: undefined },
      gridMetrics,
    );
    tiles = tiles.map((t) => (t.id === tileResize!.id ? next : t));
    applyTileToElement(tileResize.el, next, shell, gridMetrics);
    persist();
  };

  desktop.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;

    const handle = target.closest(".panel-resize-handle");
    if (handle) {
      const tileEl = handle.closest<HTMLElement>(".panel-tile");
      if (!tileEl) return;
      const id = tileEl.dataset.panelId!;
      if (!isPanelResizable(id)) return;
      tileResize = { id, el: tileEl };
      event.preventDefault();
      const resumeClickThrough = clickThrough.suspend();
      trackPointerSession(
        (moveEvent) => applyResize(moveEvent.clientX, moveEvent.clientY),
        () => {
          tileResize = null;
          persist();
          resumeClickThrough();
          syncClickThrough();
        },
      );
      return;
    }

    if (!target.closest(".panel-tile-header")) return;
    const tileEl = target.closest<HTMLElement>(".panel-tile");
    if (!tileEl) return;
    const id = tileEl.dataset.panelId!;
    const tileRect = tileEl.getBoundingClientRect();
    tileDrag = {
      id,
      el: tileEl,
      pointerOffsetX: event.clientX - tileRect.left,
      pointerOffsetY: event.clientY - tileRect.top,
    };
    event.preventDefault();
    const resumeClickThrough = clickThrough.suspend();
    trackPointerSession(
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
          tileDrag = null;
          resumeClickThrough();
          syncClickThrough();
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
        if (resolved) {
          tiles = tiles.map((t) => (t.id === tileDrag!.id ? resolved! : t));
          panelLayouts[resolved.id] = resolved;
        }
        tileDrag = null;
        persist();
        resumeClickThrough();
        syncClickThrough();
      },
    );
  });

  document.addEventListener("click", (event) => {
    const target = event.target as Element;
    if (!target.closest(".menu-group")) closeDropdowns();
  });

  document.addEventListener("pointerdown", (event) => {
    const target = event.target as Element;
    if (!target.closest(".menu-group")) closeDropdowns();
  });
}
