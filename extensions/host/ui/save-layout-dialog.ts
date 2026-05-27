export type SaveLayoutSlotOption = {
  slotIndex: number;
  label: string;
  occupied: boolean;
};

export type SaveLayoutResult = {
  slotIndex: number;
  name: string;
};

/** Centered modal for choosing a profile slot and name (supports overwrite). */
export function promptSaveLayout(
  shellRoot: HTMLElement,
  slots: SaveLayoutSlotOption[],
  defaultSlotIndex: number,
  defaultName: string,
): Promise<SaveLayoutResult | null> {
  return new Promise((resolve) => {
    const backdrop = document.createElement("div");
    backdrop.className = "shell-modal-backdrop";
    backdrop.dataset.interactive = "true";

    const dialog = document.createElement("div");
    dialog.className = "shell-modal";
    dialog.role = "dialog";
    dialog.setAttribute("aria-modal", "true");
    dialog.setAttribute("aria-labelledby", "save-layout-title");

    const slotOptions = slots
      .map(
        (slot) => `
      <label class="shell-modal-slot-option">
        <input type="radio" name="save-layout-slot" value="${slot.slotIndex}" ${
          slot.slotIndex === defaultSlotIndex ? "checked" : ""
        } />
        <span class="shell-modal-slot-label">${escapeHtml(slot.label)}</span>
        ${
          slot.occupied
            ? '<span class="shell-modal-slot-hint">overwrite</span>'
            : '<span class="shell-modal-slot-hint shell-modal-slot-hint-empty">empty</span>'
        }
      </label>`,
      )
      .join("");

    dialog.innerHTML = `
      <h2 id="save-layout-title" class="shell-modal-title">Save layout</h2>
      <p class="shell-modal-desc">Pick a slot and name for this canvas layout.</p>
      <fieldset class="shell-modal-slots">
        <legend class="shell-modal-label">Save to slot</legend>
        <div class="shell-modal-slot-list">${slotOptions}</div>
      </fieldset>
      <p class="shell-modal-overwrite" hidden></p>
      <label class="shell-modal-field">
        <span class="shell-modal-label">Layout name</span>
        <input type="text" class="shell-modal-input" maxlength="64" autocomplete="off" spellcheck="false" />
      </label>
      <div class="shell-modal-actions">
        <button type="button" class="shell-modal-btn" data-action="cancel">Cancel</button>
        <button type="button" class="shell-modal-btn shell-modal-btn-primary" data-action="save">Save</button>
      </div>
    `;

    const input = dialog.querySelector<HTMLInputElement>(".shell-modal-input")!;
    const overwriteNote = dialog.querySelector<HTMLElement>(".shell-modal-overwrite")!;
    input.value = defaultName;

    const selectedSlot = (): SaveLayoutSlotOption | undefined => {
      const checked = dialog.querySelector<HTMLInputElement>(
        'input[name="save-layout-slot"]:checked',
      );
      if (!checked) return slots[0];
      const index = Number(checked.value);
      return slots.find((slot) => slot.slotIndex === index);
    };

    const syncOverwriteNote = (): void => {
      const slot = selectedSlot();
      if (!slot?.occupied) {
        overwriteNote.hidden = true;
        overwriteNote.textContent = "";
        return;
      }
      overwriteNote.hidden = false;
      overwriteNote.textContent = `This will replace the saved layout in ${slot.label}.`;
    };

    const syncNameFromSlot = (): void => {
      const slot = selectedSlot();
      if (!slot) return;
      input.value = slot.label;
      syncOverwriteNote();
    };

    dialog.querySelectorAll<HTMLInputElement>('input[name="save-layout-slot"]').forEach(
      (radio) => {
        radio.addEventListener("change", syncNameFromSlot);
      },
    );

    backdrop.append(dialog);
    shellRoot.append(backdrop);
    syncOverwriteNote();

    let settled = false;
    const finish = (value: SaveLayoutResult | null): void => {
      if (settled) return;
      settled = true;
      document.removeEventListener("keydown", onKeyDown);
      backdrop.remove();
      resolve(value);
    };

    const commit = (): void => {
      const slot = selectedSlot();
      const trimmed = input.value.trim();
      if (!slot || !trimmed) {
        input.focus();
        input.select();
        return;
      }
      finish({ slotIndex: slot.slotIndex, name: trimmed });
    };

    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        event.preventDefault();
        finish(null);
        return;
      }
      if (event.key === "Enter" && event.target === input) {
        event.preventDefault();
        commit();
      }
    };

    backdrop.addEventListener("click", (event) => {
      if (event.target === backdrop) finish(null);
    });

    dialog.querySelector('[data-action="cancel"]')!.addEventListener("click", () => {
      finish(null);
    });
    dialog.querySelector('[data-action="save"]')!.addEventListener("click", commit);

    document.addEventListener("keydown", onKeyDown);
    requestAnimationFrame(() => {
      input.focus();
      input.select();
    });
  });
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}
