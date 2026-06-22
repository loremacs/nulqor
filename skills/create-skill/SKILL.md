---
name: create-skill
description: Creates a new skill (SKILL.md, Metadata, contract). Use when adding or scaffolding a skill, or when a repeatable workflow should become a skill artifact.
---

## Metadata

```text
skill_version: 4.0.0
applies_to:    nulqor
docs:          https://openagentskills.dev/docs/specification
topics:        meta, skills, scaffold, workflow, bootstrap
platform:      all
script_policy: optional
scope:         generic
```

`<skills-root>` = `skills/` in this repo. Format baseline: https://openagentskills.dev/docs/specification

---

## When to use

- A repeatable workflow should become a reusable skill.
- Asked explicitly to create, add, or write a skill.

Do not use for one-off fixes or unvalidated processes.

---

## Contract

```text
when:         Before creating a new skill in skills/
inputs:       skill_name, description (frontmatter only), applies_to, topics, platform, script_policy
outputs:      skills/<skill-name>/SKILL.md; scripts/ when required; skills/index.md row
side-effects: creates files; updates skills/index.md
validation:   skills/audit-skill/scripts/audit.ps1 passes; audit-project after index change
```

---

## Steps

1. Discover conventions — [REFERENCE.md](REFERENCE.md).
2. Check duplicates in `skills/<name>/` and `skills/index.md`.
3. Create `SKILL.md` — template: [references/skill-format.md](references/skill-format.md).
4. Update `skills/index.md` (`| Skill | Purpose |`).
5. Run `skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet` and `audit-project`.
6. Report via [FORMS.md](FORMS.md).

Optional: `skills/create-skill/scripts/create.ps1` or `scripts/create.sh`.

---

## Verification

- [ ] Frontmatter: only `name` and `description`.
- [ ] Body: Metadata, When to use, Contract, Steps, Verification.
- [ ] `audit-skill` and `audit-project` passed or failures reported.
