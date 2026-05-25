# audit-project

**Type:** Skill  
**Status:** Active

## Purpose

Verify file structure integrity after any file move, rename, restructure, or new extension.
Checks extension colocation, forbidden legacy paths, index/mod.rs/lib.rs sync, and runs `nulqor-lint`.

## Contract

```yaml
name: audit-project
version: 0.2.0
inputs:
  - root: repo root path (defaults to current directory)
  - skip_lint: optional, skip nulqor-lint invocation
outputs:
  - result: pass | fail
  - errors: list of integrity errors (empty on pass)
tool_loop_cap: 5
```

## Checks performed

- Required top-level dirs and area `index.md` files
- Forbidden: `src-tauri/src/ext_*.rs`, root `src/*.{ts,tsx,css}` (except `src/README.md`)
- Each `extensions/<id>/`: `extension.toml`, `src/lib.rs`, `README.md`; `ui/` for Panel kind
- Sync: disk extensions ↔ `extensions/index.md` ↔ `mod.rs` `#[path]` ↔ `lib.rs` `loader.register`
- `nulqor-lint` on `extensions/` (unless `-SkipLint`)

## Usage

Run after any file move, rename, restructure, or new extension:

```powershell
skills/audit-project/scripts/audit.ps1 [-Root <path>] [-Quiet] [-SkipLint]
```

`-Quiet` suppresses output on pass; always prints `FAIL:` lines on failure.
