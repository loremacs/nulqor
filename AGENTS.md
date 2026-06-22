# Nulqor — Agent Guide


## Repository layout

Each main content area has an `index.md` that lists what lives there. **Read the index for an area before browsing its tree or adding files.**

| Path | Index | Purpose |
|---|---|---|
| `docs/` | [`docs/index.md`](docs/index.md) | Planning, design, phases, decisions, shipped features |
| `extensions/` | [`extensions/index.md`](extensions/index.md) | One folder per extension: manifest, `src/`, `ui/` (panels) |
| `skills/` | [`skills/index.md`](skills/index.md) | Reusable agent workflows and audit scripts |
| `rules/` | [`rules/index.md`](rules/index.md) | Runtime context rules (loaded by `context-editor`) |
| `tools/` | [`tools/index.md`](tools/index.md) | Linter, MCP server, workspace dev utilities |
| `archive/` | [`archive/index.md`](archive/index.md) | Superseded docs — read-only |
| `src-tauri/src/` | `docs/DESIGN.md` §14 | Frozen core only (`loader`, `events`, `commands`, …) |
| `src-tauri/src/extensions/mod.rs` | [`extensions/index.md`](extensions/index.md) | `#[path]` bridge compiling `extensions/<id>/src/lib.rs` |
| `src/` | [`src/README.md`](src/README.md) | Placeholder only; panel UI lives under `extensions/<id>/ui/` |
| `runs/` | — | Runtime JSONL logs from `run-logger` (gitignored) |

**Root operational files (no index — read directly):**

| File | Purpose |
|---|---|
| [`TASKS.md`](TASKS.md) | Active task queue and definition of done |
| [`BACKLOG.md`](BACKLOG.md) | Ideas for later — not yet scheduled in `TASKS.md` |
| [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md) | **Chat / sessions / group chat handoff** — v1 shipped vs designed gaps |
| [`README.md`](README.md) | Build commands, prerequisites, repo map |
| [`skills/canvas-layout/SKILL.md`](skills/canvas-layout/SKILL.md) | Host grid/split layout — read before editing canvas UI |
| [`nulqor.toml`](nulqor.toml) | Startup: `open_panels`, `enabled_extensions`, `[shell]` grid options |

### How agents load context

| Tool | Read order |
|---|---|
| **External** (any IDE/CLI) | [`README.md`](README.md) → this file → area `index.md` → matching [`skills/index.md`](skills/index.md) skill |
| **In-app** (`context-editor`) | This file (persona) + all [`rules/`](rules/index.md) + compact skill index; full skill via `load_skill` |

---

## Layout contract (mechanically enforced)

`skills/audit-project/scripts/audit.ps1` fails the build on violations. Do not bypass — fix the layout.

### Extensions (colocated)

| Do | Don't |
|---|---|
| `extensions/<id>/extension.toml` | Loose manifests elsewhere |
| `extensions/<id>/src/lib.rs` | `src-tauri/src/ext_*.rs` |
| `extensions/<id>/ui/` for `kind = "Panel"` | `src/*.ts`, `src/*.css` at repo root |
| `extensions/<id>/README.md` | Extension code split across unrelated folders |
| Register in `extensions/index.md` | Orphan folders not listed in index |
| `#[path]` in `src-tauri/src/extensions/mod.rs` | Missing bridge for a disk extension |
| `loader.register("<id>", …)` in `src-tauri/src/lib.rs` | Extension on disk but never loaded |

**New extension:** run [`skills/create-extension/scripts/create.ps1`](skills/create-extension/scripts/create.ps1) — do not hand-scaffold.

### Other areas

| Area | Contract |
|---|---|
| `skills/<name>/` | `SKILL.md` (frontmatter: `name`, `description`) + entry in `skills/index.md` |
| `rules/` | `*.md` rule file + entry in `rules/index.md` |
| `docs/` | Map in `docs/index.md`; decisions in `docs/decisions/` |
| `archive/` | Read-only; never execute or treat as operational |

---

## Boundaries

**NEVER**
- Overwrite operational files (docs, plans, skills) in place — draft first
- Execute `.draft` files
- Initialize or interact with git
- Improve, refactor, or restructure code outside the task scope
- Silently ignore failed commands, lint, or test output
- Re-derive steps that an existing script already handles — run the script
- Treat `archive/` as operational — copy out only with human approval
- Create a new extension without `create-extension` script + audit pass
- Put extension Rust or panel UI outside `extensions/<id>/`

**ASK**
- Before adding a dependency to `Cargo.toml` or `package.json`
- Before a change touches more than one extension
- When requirements conflict with `docs/PHASES.md` or `docs/DESIGN.md`
- When ambiguity affects architecture, behavior, data shape, or scope — present options, don't pick silently

**ALWAYS**
- Read `docs/PHASES.md` and `TASKS.md` before starting non-trivial work
- Check [`skills/index.md`](skills/index.md) — if a skill matches the task, read its `SKILL.md` first
- To talk to a **running app** (HTTP/MCP chat), read [`skills/nulqor-communicate/SKILL.md`](skills/nulqor-communicate/SKILL.md) and run `scripts/chat.ps1`
- For code edits, read [`skills/edit-and-verify/SKILL.md`](skills/edit-and-verify/SKILL.md) and run the listed verifiers
- Read the target area's `index.md` before adding or moving files there
- Read any file before editing it — never edit from assumptions
- State a brief plan before non-trivial edits; for trivial edits, proceed and state assumptions inline
- Remove only imports, variables, or files that your own changes made unused
- Update `docs/DESIGN.md`, `docs/PHASES.md`, or `docs/decisions/` when the change affects them
- Update [`docs/PROJECT_FEATURES.md`](docs/PROJECT_FEATURES.md) when shipping a feature
- Run `skills/create-extension/scripts/create.ps1` when adding a new extension
- Run `skills/create-skill/scripts/create.ps1` when adding a new skill (or follow `skills/create-skill/SKILL.md`)
- Run `skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet` after any skill change
- Run `skills/audit-project/scripts/audit.ps1 -Quiet` after any file move, rename, restructure, or new extension
- Honor `rules/engineering-guardrails.md` (polling vs events, validation honesty, lock-across-IO, port uniqueness, doc consistency); the agent-loop iteration cap (`docs/DESIGN.md §13`) ships in the same change as the loop, with its test
- Report clearly when a command fails — never invent success
- Determine state before interacting — never assume OS, tool, app, or runtime versions; check first, then act
- When a tool call fails and a non-obvious workaround was required, flag it as a skill capture opportunity

## Where to look

- [`docs/index.md`](docs/index.md) — documentation map (design, phases, decisions)
- [`docs/GOAL.md`](docs/GOAL.md) — why Nulqor exists
- [`docs/PROJECT_FEATURES.md`](docs/PROJECT_FEATURES.md) — shipped feature record
- [`extensions/index.md`](extensions/index.md) — runtime extensions and manifests
- [`skills/index.md`](skills/index.md) — reusable agent workflows
- [`skills/nulqor-communicate/SKILL.md`](skills/nulqor-communicate/SKILL.md) — HTTP/MCP chat with running app
- [`skills/edit-and-verify/SKILL.md`](skills/edit-and-verify/SKILL.md) — edit code and run verifiers
- [`skills/create-skill/SKILL.md`](skills/create-skill/SKILL.md) — scaffold new skills under `skills/`
- [`rules/index.md`](rules/index.md) — context rules injected at runtime
- [`tools/index.md`](tools/index.md) — linter and dev utilities
- [`archive/index.md`](archive/index.md) — superseded material
- [`TASKS.md`](TASKS.md) — active task queue
- [`BACKLOG.md`](BACKLOG.md) — ideas for later; **chat/session handoff pointer at top**
- [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md) — sessions file store, thread vs room, human rail, forks (read before chat/session work)
- [`README.md`](README.md) — build commands and prerequisites

### Topic routing

| If the task touches… | Read first |
|---|---|
| Chat UI, sessions, persistence, group chat, human rail, forks | [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md) → [`docs/PROJECT_FEATURES.md`](docs/PROJECT_FEATURES.md) §2.4–2.4b → [`BACKLOG.md`](BACKLOG.md) § Chat |
| Host canvas, grid/split layout, click-through | [`skills/canvas-layout/SKILL.md`](skills/canvas-layout/SKILL.md) |
| HTTP/MCP with a running app | [`skills/nulqor-communicate/SKILL.md`](skills/nulqor-communicate/SKILL.md) |

---

## Multi-platform targeting

Nulqor targets **macOS, Windows, and Linux**. The codebase must remain cross-platform at all times.

**When developing on a specific OS, test on that OS — but never break the others.** A macOS fix that breaks `npm start` on Windows (or vice versa) is not shippable. Platform-specific behavior must be guarded; the default path must work everywhere.

**When developing on a specific OS, focus testing on that OS** — platform-specific bugs should be reproduced and fixed against the current platform before shipping.

### Dev startup (all platforms)

```powershell
npm install
npm start    # runs scripts/start-dev.mjs → tauri dev (OS-aware stale-process cleanup)
```

Do **not** put macOS-only shell commands (`lsof`, `pkill`, …) in `package.json` `start` — use `scripts/start-dev.mjs` which branches on `process.platform`.

### Platform guards

| Layer | How to branch |
|---|---|
| Rust | `#[cfg(target_os = "macos")]`, `#[cfg(target_os = "windows")]` |
| TypeScript | `isMacOS()`, `isWindows()` from `extensions/host/ui/platform.ts` |
| npm scripts | `scripts/start-dev.mjs` (Node `process.platform`) |
| CSS | `.platform-macos` class on `<html>` (set at init in `shell.ts`) |

Never add a Windows-only or macOS-only workaround as the default path — always use a platform guard and handle the fallback.

### macOS-specific notes

- **Native menu bar** (`src-tauri/src/native_menu.rs`): JS dropdowns are hidden on macOS (`display:none` via `.platform-macos .menu-bar-menus`); state changes must sync via `update_menu_check` IPC (guard all calls with `isMacOS()`).
- **Window chrome** (`extensions/host/ui/window-chrome/macos.ts`): Traffic-light buttons, leading order. See `chrome-mount.ts` for shared logic.
- **`startDragging()` deactivates the app**: On macOS, native window drag hands app activation to the system. `chrome-mount.ts` listens for `onFocusChanged` after `startDragging()` and calls `setFocus()` to restore activation. Do not call `setFocus()` on every `pointerdown` — only when `!document.hasFocus()`.
- **Panel drag / WKWebView pointer events**: Always call `setPointerCapture(event.pointerId)` on the drag handle before starting a drag session (grid and split). Without it, WKWebView may stop delivering `pointermove`/`pointerup` when the cursor passes over `pointer-events:none` areas, and leaked listeners break subsequent drags.
- **Click-through**: `setIgnoreCursorEvents` is async IPC. The 16 ms poll keeps it current; `suspend()`/`resume()` brackets any drag or edit operation. Use `pointerover` on interactive targets to pre-empt pass-through before click-to-activate swallows the first click.
- **Overlay startup**: `showShellWindow` uses extra paint ticks + resize nudge on macOS so WKWebView compositing shows the transparent shell correctly.
