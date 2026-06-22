# Nulqor

Local-first, extensible construction platform. Frozen Rust core + replaceable extensions.

**Agents:** read [`README.md`](README.md) for commands, then [`AGENTS.md`](AGENTS.md) for layout, boundaries, and index links.

**Humans:** product *why* → [`docs/GOAL.md`](docs/GOAL.md) · *how* → [`docs/DESIGN.md`](docs/DESIGN.md) · build order → [`docs/BUILD_PLAN.md`](docs/BUILD_PLAN.md) · current phase → [`docs/PHASES.md`](docs/PHASES.md)

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- Tauri 2 system deps — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Build and run

Nulqor is developed and run on **Windows, macOS, and Linux**. Use the same commands on every OS:

```powershell
npm install
npm start          # cross-platform — scripts/start-dev.mjs then tauri dev
```

`npm start` must never assume a single OS (no bare `lsof`/`pkill` in `package.json`). Platform-specific cleanup lives in `scripts/start-dev.mjs`.

**Agents:** when adding npm scripts, shell helpers, or dev tooling, branch on OS or use Node/cross-platform tools. Read [`AGENTS.md`](AGENTS.md) § Multi-platform targeting before OS-specific changes.

## Startup profile (`nulqor.toml`)

Root config selects which extensions load and which panels open on the grid desk. Restart after changing `enabled_extensions`.

```toml
open_panels = ["hello-world"]
enabled_extensions = ["host", "hello-world"]

[shell]
cell_pixels = 64
snap_enabled = true
show_grid = true
click_through = true
always_on_top = false
```

The host shell UI is always the window. Use **Apps** in the menu to open/close panel tiles live. Backend-only extensions still require restart.

```powershell
cargo test --workspace
cargo run --manifest-path tools/nulqor-lint/Cargo.toml -- extensions/
```

## Audit (after skill, layout, or extension changes)

```powershell
skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet
skills/audit-project/scripts/audit.ps1 -Quiet
```

## New extension (use the scaffold — do not hand-create)

```powershell
skills/create-extension/scripts/create.ps1 -Id <kebab-id> -Kind Service -Purpose "..."
```

## Talk to running app

```powershell
skills/nulqor-communicate/scripts/chat.ps1 -Action send -Message "Hello"
```

See [`skills/nulqor-communicate/SKILL.md`](skills/nulqor-communicate/SKILL.md) for HTTP, MCP, and WebSocket surfaces.

## Repository map

| Path | Index |
|---|---|
| `docs/` | [`docs/index.md`](docs/index.md) |
| `extensions/` | [`extensions/index.md`](extensions/index.md) |
| `skills/` | [`skills/index.md`](skills/index.md) |
| `rules/` | [`rules/index.md`](rules/index.md) |
| `tools/` | [`tools/index.md`](tools/index.md) |
| `archive/` | [`archive/index.md`](archive/index.md) |

Active task queue: [`TASKS.md`](TASKS.md)
