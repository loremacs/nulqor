---
name: mac-overlay-host
description: Fix macOS overlay host UI bugs — click-through races, grid drag failures, focus stolen by Finder, native menu sync. Use when panels won't drag or focus is lost on macOS.
---

## Metadata

```text
skill_version: 1.0.0
applies_to:    tauri@2, macos
docs:          https://v2.tauri.app/
topics:        macos, host, click-through, overlay, drag, focus, pointer-capture, wkwebview, ui
platform:      macos
script_policy: none
scope:         project-scoped
```

Nulqor's host is a transparent always-on-top overlay (WKWebView via Tauri 2). macOS focus,
click-through, and pointer-event delivery interact badly unless handled deliberately. The
`scripts/` directory is intentionally empty (instruction-only skill).

---

## When to use

- A grid/split panel drags once, then stops dragging on subsequent attempts.
- Clicking a panel switches the app to Finder and the drag fails.
- The transparent overlay renders blank/garbled at startup on macOS.
- Native macOS menu checkmarks drift from the in-app state.

Do not use for Windows/Linux drag bugs — those are not WKWebView focus issues.

---

## Contract

```text
when:         Diagnosing macOS host overlay drag, focus, click-through, or startup bugs
inputs:       the symptom; the host UI files under extensions/host/ui/
outputs:      a guarded macOS fix that preserves Windows/Linux behavior
side-effects: edits host UI / native menu / window frame code
validation:   tsc passes; drag works repeatedly on macOS; other OSes unaffected
```

---

## Root causes (check in this order)

1. **`setIgnoreCursorEvents` is async IPC.** Toggling click-through races the next native click. Re-enabling pass-through too soon lets macOS hand the click to the desktop (Finder).
2. **First-click activation.** An unfocused overlay region passes the click through, so the app loses activation mid-drag.
3. **WKWebView drops pointer events** over `pointer-events:none` regions unless the drag handle captured the pointer.
4. **`startDragging()` deactivates the app** — native window drag hands activation to the system.

---

## Rules (the fixes)

- **Pointer capture every drag.** Call `setPointerCapture(event.pointerId)` on the drag handle before a grid/split drag; release on `pointerup`/`lostpointercapture`. Without it, `pointermove`/`pointerup` stop firing. (`extensions/host/ui/shell.ts`)
- **Suspend click-through during a drag.** `clickThrough.suspend()` + `flush()` before the drag; `resume()` + `deferPassThrough()` after. (`extensions/host/ui/click-through.ts`, `shell.ts`)
- **Debounce re-enabling pass-through on macOS (~180ms)** so panel clicks are not stolen. Arm panel headers on `pointermove`; hit-test with `document.elementsFromPoint` so z-order is respected.
- **Restore activation after `startDragging()`.** Listen for `onFocusChanged`; call `setFocus()` only when `!document.hasFocus()` — never on every `pointerdown` (it fights the drag).
- **Sync the native menu.** macOS hides JS dropdowns; mirror state via `update_menu_check` IPC, guarded by `isMacOS()`. (`src-tauri/src/native_menu.rs`, `shell.ts`)
- **Overlay startup.** Use extra paint ticks plus a resize/transparency nudge on macOS so WKWebView composites the transparent shell. (`extensions/host/ui/window-frame.ts`)
- **Guard everything** with `isMacOS()` / `#[cfg(target_os = "macos")]` and keep the Windows/Linux path working — see `platform-guarded-change`.

---

## File map

| File | Responsibility |
|---|---|
| `extensions/host/ui/click-through.ts` | Pass-through toggling, debounce, arming, hit-test |
| `extensions/host/ui/shell.ts` | Grid/split drag sessions, pointer capture, focus restore |
| `extensions/host/ui/window-chrome/macos.ts` + `chrome-mount.ts` | Traffic-light chrome, drag, focus restore |
| `extensions/host/ui/window-frame.ts` | Transparent startup paint/nudge, frame persistence |
| `extensions/host/ui/platform.ts` | `isMacOS()` / `isWindows()` |
| `src-tauri/src/native_menu.rs` | Native menu + `update_menu_check` |

---

## Verification

- [ ] A grid panel drags repeatedly (5+ times) without losing focus to Finder.
- [ ] Clicking a panel does not switch the app to Finder.
- [ ] Native menu checkmarks match in-app state after toggling.
- [ ] Overlay renders correctly at startup.
- [ ] `npx tsc --noEmit` passes; Windows/Linux drag still works (no regression).
