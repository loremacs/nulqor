# Skills Index

Reusable agent workflows. Each skill has a `SKILL.md` with a contract block.
Skills without a contract block are guides (instructional only — never executed).

Read a skill's `SKILL.md` before using it. Check this index first.

## Available Skills

| Skill | Type | Purpose |
|---|---|---|
| `audit-skill` | Skill | Validate a skill's `SKILL.md` against the contract schema |
| `audit-project` | Skill | Verify file structure integrity after moves/renames/restructures |

## Adding a Skill

1. Create `skills/<kebab-name>/SKILL.md` with the standard contract block.
2. Run `skills/audit-skill/scripts/audit.ps1 -Quiet` to validate it.
3. Add an entry to this index.
