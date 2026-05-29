import { getCurrentWindow } from "@tauri-apps/api/window";

type ResizeDirection =
  | "East"
  | "North"
  | "NorthEast"
  | "NorthWest"
  | "South"
  | "SouthEast"
  | "SouthWest"
  | "West";

const HANDLES: Array<{ dir: ResizeDirection; className: string }> = [
  { dir: "North", className: "window-resize-n" },
  { dir: "South", className: "window-resize-s" },
  { dir: "East", className: "window-resize-e" },
  { dir: "West", className: "window-resize-w" },
  { dir: "NorthEast", className: "window-resize-ne" },
  { dir: "NorthWest", className: "window-resize-nw" },
  { dir: "SouthEast", className: "window-resize-se" },
  { dir: "SouthWest", className: "window-resize-sw" },
];

export type WindowResizeController = {
  setEnabled: (enabled: boolean) => void;
  dispose: () => void;
};

export function mountWindowResize(isWindowed: () => boolean): WindowResizeController {
  const layer = document.createElement("div");
  layer.className = "window-resize-layer";
  layer.hidden = true;

  for (const handle of HANDLES) {
    const el = document.createElement("div");
    el.className = `window-resize-handle ${handle.className}`;
    el.addEventListener("pointerdown", (event) => {
      if (!isWindowed()) return;
      event.preventDefault();
      event.stopPropagation();
      void getCurrentWindow()
        .startResizeDragging(handle.dir)
        .catch((err) => {
          console.warn("[window-resize] startResizeDragging failed:", err);
        });
    });
    layer.appendChild(el);
  }

  document.body.appendChild(layer);

  return {
    setEnabled(enabled: boolean) {
      layer.hidden = !enabled;
    },
    dispose() {
      layer.remove();
    },
  };
}
