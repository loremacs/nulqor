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
version:       1.0.0
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

Do not repeat `name`, `version`, `topics`, or `platform` in Contract — those live in
frontmatter and `## Metadata`.

---

## Progressive disclosure budget

| Stage | Content | Target |
|---|---|---|
| L1 | `name` + `description` (YAML only) | ~100 tokens |
| L2 | Full body after `---` | < 500 lines |
| L3 | Sibling files | on reference only |
