---
name: platform-guarded-change
description: Make an OS-specific change to Nulqor without breaking other platforms. Use when editing code that behaves differently on Windows, macOS, or Linux.
---

## Metadata

```text
skill_version: 1.0.0
applies_to:    nulqor
topics:        platform, cross-platform, guards, windows, macos, linux, workflow
platform:      all
script_policy: none
scope:         project-scoped
```

Nulqor ships on Windows, macOS, and Linux from one codebase. An OS-specific fix must never
become the default path that breaks the others.
The `scripts/` directory is intentionally empty (instruction-only skill).

---

## When to use

- A change behaves differently per OS (window chrome, focus, paths, process management).
- Adding platform-conditional logic in Rust, TypeScript, CSS, or an npm script.
- A reported bug reproduces on only one OS.

Do not use for OS-agnostic logic — no guard is needed there.

---

## Contract

```text
when:         Before landing code whose behavior differs by operating system
inputs:       the change; the target OS; the OS you can test on now
outputs:      a platform-guarded edit; default path still works on every OS
side-effects: edits source; may touch scripts/start-dev.mjs
validation:   tsc + cargo check pass; npm start works on the current OS; guards present
```

---

## Steps

1. Identify the layer and use its guard — never hardcode an OS-only path as the default:

   | Layer | Guard |
   |---|---|
   | Rust | `#[cfg(target_os = "macos")]`, `#[cfg(target_os = "windows")]` |
   | TypeScript | `isMacOS()`, `isWindows()` from `extensions/host/ui/platform.ts` |
   | npm scripts | branch on `process.platform` in `scripts/start-dev.mjs` (never put `lsof`/`pkill` in `package.json`) |
   | CSS | `.platform-macos` class on `<html>` (set in `shell.ts`) |

2. Keep a working default for the non-targeted OSes. The guarded branch is the exception; the fallback must run everywhere.
3. Implement the change behind the guard.
4. Verify on the current OS: `npx tsc --noEmit`, `cargo check` (in `src-tauri/`), and `npm start`.
5. Confirm the default path still holds for the other OSes — read the fallback; no OS-only API on the shared path.
6. If the change is macOS host UI (drag/focus/overlay), follow `mac-overlay-host`.
7. Update `AGENTS.md` "Multi-platform targeting" when the change adds a new platform-specific behavior.

---

## Verification

- [ ] Every OS-specific branch is behind a guard (`#[cfg]`, `isMacOS()`/`isWindows()`, `process.platform`, or `.platform-macos`).
- [ ] The default/fallback path runs on all three OSes.
- [ ] `tsc` and `cargo check` pass; `npm start` works on the current OS.
- [ ] No macOS/Linux-only shell command added to `package.json` `start`.
