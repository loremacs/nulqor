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
- [`README.md`](README.md) — build commands and prerequisites
