# Default skill format

Template for new skills when the host has no separate format doc.
Superset of https://openagentskills.dev/docs/specification

---

## Default template

```markdown
---
name: <skill-name>
description: >
  <what it does AND when to trigger it. Include indirect phrasings.
  Max 1024 characters.>
---

## Metadata

\```text
skill_version: 1.0.0
applies_to:    <software@version, comma-separated, or "nulqor" for internal>
docs:          <official doc URL matched to applies_to version, or omit>
topics:        <category>, <keyword>
platform:      <all|windows|macos|linux|combo>
script_policy: <none|optional|required>
scope:         <generic|os-scoped|tool-scoped|domain-scoped|project-scoped>
\```

<One or two sentences: what problem this skill solves.>

---

## When to use

- <trigger condition>

Do not use for: <optional out-of-scope note>

---

## Contract

\```text
when:         <trigger expansion>
inputs:       <param> -- <description, or "none">
outputs:      <what is produced>
side-effects: <changes or "none">
validation:   <observable checks before reporting success>
\```

---

## Steps

1. <step>

---

## Verification

- [ ] <observable condition>
```

Save as: `<skills-root>/<skill-name>/SKILL.md`

---

## Required sections

| Section | Required |
|---|---|
| YAML frontmatter (`name`, `description` only) | yes |
| `## Metadata` | yes |
| `## When to use` | yes |
| `## Contract` | yes |
| `## Steps` | yes |
| `## Verification` | yes |

---

## Optional standalone files

| Path | Purpose |
|---|---|
| `FORMS.md` | Input / output templates |
| `REFERENCE.md` | Deep reference (L3) |
| `EXAMPLES.md` | Worked examples (L3) |
| `scripts/` | Executables (L3) |
| `references/` | Extra docs (L3) |
| `assets/` | Templates, static files |
| `data/` | JSON, lookup tables |

---

## Contract rules

Required keys: `when`, `inputs`, `outputs`, `side-effects`, `validation`.

Do not repeat `name`, `skill_version`, `applies_to`, `topics`, or `platform` in Contract —
those live in frontmatter and `## Metadata`.

---

## Two version axes

Skills carry two independent versions so a small model can tell "which thing this documents"
apart from "which revision of our advice this is":

| Field | Meaning | Example |
|---|---|---|
| `applies_to` | The software/framework/OS + version this skill documents (the *external* version). Use `nulqor` for internal process skills. | `tauri@2`, `macos@14+`, `nulqor` |
| `docs` | Official documentation URL matched to the `applies_to` version. Optional; omit if none. | `https://v2.tauri.app/` |
| `skill_version` | Our own revision of this skill's guidance (semver). Bump when you change the steps. | `1.0.0` |

**Folder-name version suffix (opt-in).** When a skill documents an externally versioned thing
and multiple versions must coexist (e.g. legacy library docs), put the prime version in the
*folder name* — never in the `SKILL.md` filename (the loader and audit require `SKILL.md`):

\```text
skills/react-router-v6/SKILL.md   ← applies_to: react-router@6
skills/react-router-v5/SKILL.md   ← legacy, applies_to: react-router@5
\```

Internal/process skills get no suffix.

---

## Progressive disclosure budget

| Stage | Content | Target |
|---|---|---|
| L1 | `name` + `description` (YAML only) | ~100 tokens |
| L2 | Full body after `---` | < 500 lines |
| L3 | Sibling files | on reference only |
