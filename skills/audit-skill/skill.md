# audit-skill

**Type:** Skill  
**Status:** Stub — not yet implemented

## Purpose

Validate a skill's `SKILL.md` against the Nulqor skill contract schema.

## Contract

```yaml
name: audit-skill
version: 0.1.0
inputs:
  - skill_path: path to the skill directory containing SKILL.md
outputs:
  - result: pass | fail
  - errors: list of validation errors (empty on pass)
tool_loop_cap: 3
```

## Usage

Run after creating or editing any skill:

```powershell
skills/audit-skill/scripts/audit.ps1 -SkillPath skills/<name> [-Quiet]
```

`-Quiet` suppresses output on pass; always prints on fail.
