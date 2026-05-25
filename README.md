# Nulqor

Local-first, extensible construction platform. Frozen Rust core + replaceable extensions.

**Agents:** start at [`AGENTS.md`](AGENTS.md) — repo layout, boundaries, and index links.

**Humans:** product *why* → [`docs/GOAL.md`](docs/GOAL.md) · *how* → [`docs/DESIGN.md`](docs/DESIGN.md) · build order → [`docs/BUILD_PLAN.md`](docs/BUILD_PLAN.md) · current phase → [`docs/PHASES.md`](docs/PHASES.md)

---

## Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 18+
- Tauri 2 system deps — see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)

## Build and run

```powershell
npm install
npm start          # tauri dev — opens window titled "Nulqor"
```

## Startup profile (`nulqor.toml`)

Root config selects which extensions load and which panels open on the grid desk. Restart after changing `enabled_extensions`.

```toml
open_panels = ["hello-world"]
enabled_extensions = ["host", "hello-world"]

[shell]
grid_cols = 12
grid_rows = 8
snap_enabled = true
show_grid = true
```

The host shell UI is always the window. Use **Apps** in the menu to open/close panel tiles live. Backend-only extensions still require restart.

```powershell
cargo test --workspace
cargo run --manifest-path tools/nulqor-lint/Cargo.toml -- extensions/
```

## Audit (after skill, layout, or extension changes)

```powershell
skills/audit-skill/scripts/audit.ps1 -SkillPath skills/<name> -Quiet
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

See [`skills/nulqor-communicate/skill.md`](skills/nulqor-communicate/skill.md) for HTTP, MCP, and WebSocket surfaces.

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
