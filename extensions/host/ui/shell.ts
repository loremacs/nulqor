import { invoke } from "@tauri-apps/api/core";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

import {
  applyMenuLayout,
  clampTileToDesk,
  nearestMenuDock,
  pointerToGridCell,
  retilePreservingPixels,
  tileSnapRect,
  updateGridGeometry,
  type GridMetrics,
} from "./grid";
import { isPanelResizable, mountPanel, registeredPanelIds } from "./panels";
import {
  DEFAULT_SHELL,
  STORAGE_KEY,
  type CanvasConfig,
  type MenuDock,
  type PersistedShellState,
  type ShellConfig,
  type TileLayout,
} from "./types";

const DEFAULT_TILE_COLS = 4;
const DEFAULT_TILE_ROWS = 3;
const MIN_TILE_COLS = 1;
const MIN_TILE_ROWS = 1;

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
      parsed.shell.cell_pixels = Math.min(
        256,
        Math.max(16, Number(parsed.shell.cell_pixels) || DEFAULT_SHELL.cell_pixels),
      );
    }
    if (!parsed.panelLayouts && parsed.tiles) {
      parsed.panelLayouts = Object.fromEntries(parsed.tiles.map((t) => [t.id, t]));
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

  if (!shell.snap_enabled && tile.freeX !== undefined && tile.freeY !== undefined) {
    const { width, height } = tileSnapRect(tile, metrics);
    el.classList.add("panel-tile-free");
    el.style.position = "absolute";
    el.style.left = `${tile.freeX}px`;
    el.style.top = `${tile.freeY}px`;
    el.style.width = `${width}px`;
    el.style.height = `${height}px`;
    return;
  }

  const snapped = tileSnapRect(tile, metrics);
  el.classList.add("panel-tile-snap");
  el.style.position = "absolute";
  el.style.left = `${snapped.left}px`;
  el.style.top = `${snapped.top}px`;
  el.style.width = `${snapped.width}px`;
  el.style.height = `${snapped.height}px`;
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

function isMenuDragBlocked(target: HTMLElement): boolean {
  return Boolean(
    target.closest(".menu-btn, .menu-window-btn, .menu-window-controls, .menu-dropdown, input, label"),
  );
}

export async function initShell(): Promise<void> {
  const app = document.getElementById("app");
  if (!app) throw new Error("#app not found");

  const canvasConfig = await fetchCanvasConfig();
  const persisted = loadPersisted();

  let shell: ShellConfig = { ...DEFAULT_SHELL, ...canvasConfig.shell };
  if (persisted?.shell) shell = { ...shell, ...persisted.shell };

  const panelLayouts: Record<string, TileLayout> = { ...persisted?.panelLayouts };
  let openPanelIds: string[] =
    persisted?.openPanelIds?.filter((id) => panelLayouts[id] || canvasConfig.open_panels.includes(id)) ??
    [];

  if (openPanelIds.length === 0) {
    openPanelIds = [...canvasConfig.open_panels];
  }

  let menuDock: MenuDock = persisted?.menuDock ?? "top";
  let windowMode: "fullscreen" | "windowed" = "fullscreen";

  app.innerHTML = `
    <div class="nulqor-shell" data-menu-dock="${menuDock}">
      <header class="menu-bar" data-interactive data-dock="${menuDock}">
        <div class="menu-bar-brand" title="Drag to move menu bar">Nulqor</div>
        <nav class="menu-bar-items" data-interactive>
          <div class="menu-group">
            <button type="button" class="menu-btn" data-menu="settings">Settings</button>
            <div class="menu-dropdown" data-panel="settings" hidden>
              <label>Cell size (px) <input type="number" min="16" max="256" data-setting="cell_pixels" /></label>
              <label><input type="checkbox" data-setting="snap_enabled" /> Snap to grid</label>
              <label><input type="checkbox" data-setting="show_grid" /> Show grid lines</label>
            </div>
          </div>
          <div class="menu-group">
            <button type="button" class="menu-btn" data-menu="apps">Apps</button>
            <div class="menu-dropdown" data-panel="apps" hidden></div>
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
  const settingsPanel = app.querySelector<HTMLElement>('[data-panel="settings"]')!;
  const appsPanel = app.querySelector<HTMLElement>('[data-panel="apps"]')!;

  let gridMetrics: GridMetrics;

  const refreshGrid = (): void => {
    gridMetrics = updateGridGeometry(shellRoot, desktop, shell);
  };

  const setMenuDock = (dock: MenuDock): void => {
    menuDock = dock;
    menuBar.dataset.dock = dock;
    shellRoot.dataset.menuDock = dock;
    applyMenuLayout(shellRoot, dock);
    refreshGrid();
  };

  applyShellCss(shellRoot, shell);
  setMenuDock(menuDock);

  let tiles: TileLayout[] = openPanelIds.map((id, i) =>
    clampTile(panelLayouts[id] ?? defaultTile(id, i, gridMetrics), gridMetrics),
  );
  tiles.forEach((t) => {
    panelLayouts[t.id] = t;
  });

  const syncTilesFromLayouts = (): void => {
    tiles = openPanelIds.map((id, i) =>
      clampTile(panelLayouts[id] ?? defaultTile(id, i, gridMetrics), gridMetrics),
    );
  };

  const persist = (): void => {
    tiles.forEach((t) => {
      panelLayouts[t.id] = t;
    });
    savePersisted({ menuDock, shell, panelLayouts, openPanelIds });
  };

  const renderAppsMenu = (): void => {
    appsPanel.innerHTML = "";
    for (const panel of canvasConfig.panels) {
      const open = openPanelIds.includes(panel.id);
      const hasLoader = registeredPanelIds().includes(panel.id);
      const label = document.createElement("label");
      label.className = "apps-item";
      const input = document.createElement("input");
      input.type = "checkbox";
      input.checked = open;
      input.disabled = !hasLoader;
      input.dataset.panelId = panel.id;
      label.append(input, document.createTextNode(` ${panel.id}${hasLoader ? "" : " (restart only)"}`));
      appsPanel.append(label);
    }
    if (canvasConfig.panels.length === 0) {
      appsPanel.textContent = "No Panel extensions enabled.";
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
      let el = desktop.querySelector<HTMLElement>(`[data-panel-id="${tile.id}"]`);
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
  };

  const syncSettingsInputs = (): void => {
    settingsPanel.querySelector<HTMLInputElement>('[data-setting="cell_pixels"]')!.value =
      String(shell.cell_pixels);
    settingsPanel.querySelector<HTMLInputElement>('[data-setting="snap_enabled"]')!.checked =
      shell.snap_enabled;
    settingsPanel.querySelector<HTMLInputElement>('[data-setting="show_grid"]')!.checked =
      shell.show_grid;
  };

  syncSettingsInputs();
  renderAppsMenu();
  await renderTiles();

  const closeDropdowns = (): void => {
    setDropdownOpen(settingsPanel, false);
    setDropdownOpen(appsPanel, false);
  };

  app.querySelector('[data-menu="settings"]')!.addEventListener("click", (e) => {
    e.stopPropagation();
    const willOpen = settingsPanel.hidden;
    closeDropdowns();
    setDropdownOpen(settingsPanel, willOpen);
  });

  app.querySelector('[data-menu="apps"]')!.addEventListener("click", (e) => {
    e.stopPropagation();
    const willOpen = appsPanel.hidden;
    closeDropdowns();
    setDropdownOpen(appsPanel, willOpen);
  });

  settingsPanel.addEventListener("change", (event) => {
    const target = event.target as HTMLInputElement;
    const key = target.dataset.setting as keyof ShellConfig | undefined;
    if (!key) return;
    if (key === "snap_enabled" || key === "show_grid") {
      shell[key] = target.checked;
    } else {
      shell[key] = Number(target.value);
    }
    applyShellCss(shellRoot, shell);
    syncSettingsInputs();

    const prevMetrics = gridMetrics;
    refreshGrid();

    if (key === "cell_pixels") {
      tiles = tiles.map((t) =>
        retilePreservingPixels(t, prevMetrics, gridMetrics, shell.snap_enabled),
      );
    } else if (shell.snap_enabled) {
      tiles = tiles.map((t) => {
        if (t.freeX !== undefined && t.freeY !== undefined) {
          const origin = desktop.getBoundingClientRect();
          const cell = pointerToGridCell(origin.left + t.freeX, origin.top + t.freeY, desktop, gridMetrics);
          return clampTile(
            { ...t, col: cell.col, row: cell.row, freeX: undefined, freeY: undefined },
            gridMetrics,
          );
        }
        return clampTile({ ...t, freeX: undefined, freeY: undefined }, gridMetrics);
      });
    } else {
      tiles = tiles.map((t) => {
        if (t.freeX !== undefined && t.freeY !== undefined) return clampTile(t, gridMetrics);
        const rect = tileSnapRect(t, gridMetrics);
        return clampTile({ ...t, freeX: rect.left, freeY: rect.top }, gridMetrics);
      });
    }

    tiles.forEach((t) => {
      panelLayouts[t.id] = t;
    });
    void renderTiles();
  });

  const restoreBtn = app.querySelector<HTMLButtonElement>('[data-action="restore"]')!;

  const syncWindowMode = async (): Promise<void> => {
    const win = getCurrentWindow();
    const fullscreen = await win.isFullscreen();
    windowMode = fullscreen ? "fullscreen" : "windowed";
    shellRoot.dataset.windowMode = windowMode;
    restoreBtn.title = fullscreen ? "Restore down" : "Maximize";
    restoreBtn.textContent = fullscreen ? "❐" : "□";
    const brand = menuBar.querySelector<HTMLElement>(".menu-bar-brand")!;
    brand.title = fullscreen ? "Drag to move menu bar" : "Drag to move window";
  };

  void syncWindowMode().catch((err) => {
    console.warn("[host] syncWindowMode failed:", err);
  });

  app.querySelector('[data-action="minimize"]')!.addEventListener("click", () => {
    void getCurrentWindow().minimize();
  });

  restoreBtn.addEventListener("click", () => {
    void (async () => {
      const win = getCurrentWindow();
      const fullscreen = await win.isFullscreen();
      if (fullscreen) {
        windowMode = "windowed";
        await win.setFullscreen(false);
        await win.setResizable(true);
        await win.setSize(new LogicalSize(1280, 720));
        await win.center();
      } else {
        windowMode = "fullscreen";
        await win.setFullscreen(true);
        await win.setResizable(false);
      }
      await syncWindowMode();
      refreshGrid();
      void renderTiles();
    })();
  });

  app.querySelector('[data-action="close"]')!.addEventListener("click", () => {
    void getCurrentWindow().close();
  });

  void getCurrentWindow()
    .onResized(() => {
      refreshGrid();
      tiles = tiles.map((t) => clampTile(t, gridMetrics));
      void renderTiles();
      void syncWindowMode();
    })
    .catch((err) => {
      console.warn("[host] onResized unavailable:", err);
    });

  window.addEventListener("resize", () => {
    refreshGrid();
    tiles = tiles.map((t) => clampTile(t, gridMetrics));
    void renderTiles();
  });

  appsPanel.addEventListener("change", async (event) => {
    const target = event.target as HTMLInputElement;
    const panelId = target.dataset.panelId;
    if (!panelId) return;
    if (target.checked) {
      if (!openPanelIds.includes(panelId)) {
        openPanelIds.push(panelId);
        if (!panelLayouts[panelId]) {
          panelLayouts[panelId] = defaultTile(panelId, openPanelIds.length - 1, gridMetrics);
        }
      }
    } else {
      panelLayouts[panelId] = tiles.find((t) => t.id === panelId) ?? panelLayouts[panelId];
      openPanelIds = openPanelIds.filter((id) => id !== panelId);
    }
    syncTilesFromLayouts();
    await renderTiles();
    renderAppsMenu();
  });

  desktop.addEventListener("click", async (event) => {
    const closeId = (event.target as HTMLElement).dataset.close;
    if (!closeId) return;
    panelLayouts[closeId] = tiles.find((t) => t.id === closeId) ?? panelLayouts[closeId];
    openPanelIds = openPanelIds.filter((id) => id !== closeId);
    syncTilesFromLayouts();
    await renderTiles();
    renderAppsMenu();
  });

  menuBar.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    if (isMenuDragBlocked(target)) return;

    if (windowMode === "windowed") {
      event.preventDefault();
      void getCurrentWindow().startDragging();
      return;
    }

    event.preventDefault();
    trackPointerSession(
      () => {},
      (endEvent) => {
        setMenuDock(nearestMenuDock(endEvent.clientX, endEvent.clientY));
        persist();
      },
    );
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

  const positionTileFromPointer = (clientX: number, clientY: number): TileLayout | null => {
    if (!tileDrag) return null;
    const tile = tiles.find((t) => t.id === tileDrag!.id);
    if (!tile) return null;

    if (!shell.snap_enabled) {
      const origin = desktopOrigin();
      const freeX = clientX - tileDrag.pointerOffsetX - origin.left;
      const freeY = clientY - tileDrag.pointerOffsetY - origin.top;
      return { ...tile, freeX, freeY };
    }

    const topLeftX = clientX - tileDrag.pointerOffsetX;
    const topLeftY = clientY - tileDrag.pointerOffsetY;
    const cell = pointerToGridCell(topLeftX, topLeftY, desktop, gridMetrics);
    return clampTile({ ...tile, col: cell.col, row: cell.row, freeX: undefined, freeY: undefined }, gridMetrics);
  };

  const applyResize = (clientX: number, clientY: number): void => {
    if (!tileResize) return;
    const tile = tiles.find((t) => t.id === tileResize!.id);
    if (!tile) return;
    const endCell = pointerToGridCell(clientX, clientY, desktop, gridMetrics);
    const colSpan = Math.max(MIN_TILE_COLS, endCell.col - tile.col + 1);
    const rowSpan = Math.max(MIN_TILE_ROWS, endCell.row - tile.row + 1);
    const next = clampTile({ ...tile, colSpan, rowSpan }, gridMetrics);
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
      trackPointerSession(
        (moveEvent) => applyResize(moveEvent.clientX, moveEvent.clientY),
        () => {
          tileResize = null;
          persist();
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
    trackPointerSession(
      (moveEvent) => {
        const next = positionTileFromPointer(moveEvent.clientX, moveEvent.clientY);
        if (next) applyTileToElement(tileDrag!.el, next, shell, gridMetrics);
      },
      (endEvent) => {
        const next = positionTileFromPointer(endEvent.clientX, endEvent.clientY);
        if (next) {
          tiles = tiles.map((t) => (t.id === tileDrag!.id ? next : t));
          panelLayouts[next.id] = next;
        }
        tileDrag = null;
        persist();
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
