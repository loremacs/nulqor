# Skills Index

Reusable agent workflows. Each skill has a `SKILL.md` or `skill.md` with a contract block.
Skills without a contract block are guides (instructional only — never executed).

Read a skill's skill file before using it. Check this index first.

## Layout contract

- One skill per folder: `skills/<kebab-name>/`
- Skill file: `skill.md` or `SKILL.md`
- Scripts: `skills/<name>/scripts/*.ps1`
- **Every new skill must be listed in this index** (audit-project checks index files exist)

## Available Skills

| Skill | Type | Purpose |
|---|---|---|
| `audit-skill` | Skill | Validate a skill's contract file against the schema |
| `audit-project` | Skill | Verify repo layout, extension colocation, registry sync, and run nulqor-lint |
| `create-extension` | Skill | Scaffold `extensions/<id>/` with colocated manifest, Rust, and optional UI |
| `nulqor-communicate` | Skill | Talk to running app: HTTP API, MCP, observer protocol (`scripts/chat.ps1`) |

## Adding a Skill

1. Create `skills/<kebab-name>/skill.md` with the standard contract block.
2. Run `skills/audit-skill/scripts/audit.ps1 -Quiet` to validate it.
3. Add an entry to this index.
4. Run `skills/audit-project/scripts/audit.ps1 -Quiet`.

## Adding an Extension (not a skill — use create-extension)

Do **not** hand-create extension folders. Run:

```powershell
skills/create-extension/scripts/create.ps1 -Id <kebab-id> -Kind Service -Purpose "..."
```

See [`create-extension/skill.md`](create-extension/skill.md).
