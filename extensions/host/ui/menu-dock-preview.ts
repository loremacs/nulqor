import { menuBarThicknessPx, menuDockSnapTarget } from "./grid";
import type { MenuDock } from "./types";

export type MenuDockPreviewHandle = {
  updateFromPointer: (clientX: number, clientY: number) => void;
  hide: () => void;
};

/** Ghost outline of the menu bar strip at the snap target edge while dragging. */
export function mountMenuDockPreview(
  shellRoot: HTMLElement,
): MenuDockPreviewHandle {
  const el = document.createElement("div");
  el.className = "menu-dock-preview";
  el.setAttribute("aria-hidden", "true");
  el.hidden = true;
  shellRoot.append(el);

  const show = (dock: MenuDock): void => {
    el.dataset.dock = dock;
    el.style.setProperty(
      "--preview-thickness",
      `${menuBarThicknessPx(dock)}px`,
    );
    el.hidden = false;
    requestAnimationFrame(() => {
      el.classList.add("is-visible");
    });
  };

  const hide = (): void => {
    el.classList.remove("is-visible");
    delete el.dataset.dock;
    el.hidden = true;
  };

  const updateFromPointer = (clientX: number, clientY: number): void => {
    const dock = menuDockSnapTarget(clientX, clientY);
    if (!dock) {
      hide();
      return;
    }
    show(dock);
  };

  return { updateFromPointer, hide };
}
