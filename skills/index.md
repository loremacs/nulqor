# Skills Index

Reusable agent workflows. Each skill has `SKILL.md` with YAML frontmatter (`name`, `description`).

Read a skill's `SKILL.md` before using it. Check this index first. In-app agents see a compact
index in the system prompt and load full bodies via `load_skill(name)`.

## Layout contract

- One skill per folder: `skills/<kebab-name>/`
- Skill file: `SKILL.md` with frontmatter (see `docs/decisions/006-http-api-and-observer-protocol.md` Â§7)
- Scripts: `skills/<name>/scripts/*.ps1`
- **Every new skill must be listed in this index** (audit-project checks index files exist)

## Available Skills

| Skill | Purpose |
|---|---|
| `audit-skill` | Structural lint for skills (frontmatter, Metadata, Contract, index) |
| `audit-project` | Verify repo layout, extension colocation, registry sync, and run nulqor-lint |
| `create-extension` | Scaffold `extensions/<id>/` with colocated manifest, Rust, and optional UI |
| `create-skill` | Scaffold `skills/<name>/` with SKILL.md frontmatter, Metadata block, and index row |
| `edit-and-verify` | Edit code safely â€” read first, run tsc/cargo/audits, report results |
| `canvas-layout` | Host grid/split/sub-grid layout UI â€” invariants, file map, test checklist |
| `nulqor-communicate` | Talk to running app: HTTP API, MCP, observer protocol (`scripts/chat.ps1`) |
| `platform-guarded-change` | Make an OS-specific change without breaking Windows/macOS/Linux (guards + verify) |
| `mac-overlay-host` | macOS overlay host fixes — click-through, panel drag, focus, native menu, startup |

## Adding a Skill

1. Prefer `skills/create-skill/scripts/create.ps1` or follow [`create-skill/SKILL.md`](create-skill/SKILL.md).
2. Create `skills/<kebab-name>/SKILL.md` with `name` and `description` in YAML frontmatter plus a `## Metadata` body block. Metadata carries two version axes: `applies_to` (the software/OS+version the skill documents, e.g. `tauri@2`, or `nulqor` for internal) and `skill_version` (semver for our own revisions). See [`create-skill/references/skill-format.md`](create-skill/references/skill-format.md).
3. Run `skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet`.
4. Add an entry to this index (if not created by the script).
5. Run `skills/audit-project/scripts/audit.ps1 -Quiet`.

## Adding an Extension (not a skill â€” use create-extension)

Do **not** hand-create extension folders. Run:

```powershell
skills/create-extension/scripts/create.ps1 -Id <kebab-id> -Kind Service -Purpose "..."
```

See [`create-extension/SKILL.md`](create-extension/SKILL.md).
