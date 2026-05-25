# audit-project

**Type:** Skill  
**Status:** Stub — not yet implemented

## Purpose

Verify file structure integrity after any file move, rename, or project restructure.
Checks that index files are up to date, no broken references exist, and directory
depth rules (DESIGN.md §10) are satisfied.

## Contract

```yaml
name: audit-project
version: 0.1.0
inputs:
  - root: repo root path (defaults to current directory)
outputs:
  - result: pass | fail
  - errors: list of integrity errors (empty on pass)
tool_loop_cap: 5
```

## Usage

Run after any file move, rename, or restructure:

```powershell
skills/audit-project/scripts/audit.ps1 [-Root <path>] [-Quiet]
```

`-Quiet` suppresses output on pass; always prints on fail.
