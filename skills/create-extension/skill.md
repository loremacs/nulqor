# create-extension

**Type:** Skill  
**Status:** Active

## Purpose

Scaffold a new extension under `extensions/<id>/` using the colocated layout (manifest + `src/lib.rs` + optional `ui/`). Updates `mod.rs` bridge and `extensions/index.md`. Never hand-create extension folders without this script.

## Contract

```yaml
name: create-extension
version: 0.1.0
inputs:
  - id: kebab-case extension id (folder name)
  - kind: Service | Panel | Host | Provider
  - purpose: one-line description for README and index
  - requires: optional comma-separated extension ids
outputs:
  - scaffold_path: extensions/<id>/
  - follow_up: loader.register line to add in src-tauri/src/lib.rs
tool_loop_cap: 5
```

## Usage

Before creating any new extension:

```powershell
skills/create-extension/scripts/create.ps1 `
  -Id my-extension `
  -Kind Service `
  -Purpose "Short description of what it does"
```

Panel with dependencies:

```powershell
skills/create-extension/scripts/create.ps1 `
  -Id my-panel `
  -Kind Panel `
  -Purpose "Dashboard for X" `
  -Requires "host,transcript"
```

Then:

1. Add the printed `loader.register(...)` line to `src-tauri/src/lib.rs` in `load_extensions()`.
2. Implement commands/events in `extensions/<id>/src/lib.rs`.
3. For `Panel` kind, implement `extensions/<id>/ui/`.
4. Run `skills/audit-project/scripts/audit.ps1 -Quiet`.
5. Run `cargo test --workspace`.

## Layout contract (enforced by audit-project)

- All extension Rust → `extensions/<id>/src/lib.rs`
- Panel UI → `extensions/<id>/ui/`
- Never add `src-tauri/src/ext_*.rs` or root `src/*.ts`
- Registry must stay in sync: disk ↔ `extensions/index.md` ↔ `mod.rs` ↔ `lib.rs` loader
