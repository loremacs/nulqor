import type { PanelMount } from "./types";

export type PanelEntry = {
  load: () => Promise<PanelMount>;
  /** When false, tile size is fixed by the extension. Default true. */
  resizable?: boolean;
};

/** Panel id → loader + options. Add entries when new Panel extensions ship. */
export const PANEL_REGISTRY: Record<string, PanelEntry> = {
  "hello-world": {
    load: () => import("../../hello-world/ui/panel.ts"),
    resizable: true,
  },
  "clock-panel": {
    load: () => import("../../clock-panel/ui/panel.ts"),
    resizable: true,
  },
};

export async function mountPanel(id: string, container: HTMLElement): Promise<boolean> {
  const entry = PANEL_REGISTRY[id];
  if (!entry) {
    container.textContent = `Panel "${id}" has no UI loader yet.`;
    return false;
  }
  const mod = await entry.load();
  mod.mount(container);
  return true;
}

export function registeredPanelIds(): string[] {
  return Object.keys(PANEL_REGISTRY);
}

export function isPanelResizable(id: string): boolean {
  const entry = PANEL_REGISTRY[id];
  if (!entry) return false;
  return entry.resizable !== false;
}
