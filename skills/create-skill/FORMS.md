# Forms — create-skill

Use before writing files. Use the post-creation template in the final report.

---

## Skill metadata intake

Frontmatter (YAML only):

```
name:           [lowercase-hyphenated; matches folder name]
description:    [what + when; no padding]
```

Body — ## Metadata block:

```
version:        [semver, e.g. 1.0.0]
topics:         [comma-separated]
platform:       [all | windows | macos | linux | combo]
script_policy:  [none | optional | required]
scope:          [generic | os-scoped | tool-scoped | domain-scoped | project-scoped]
```

Optional files:

```
has_forms:      [yes | no]
has_reference:  [yes | no]
has_examples:   [yes | no]
```

---

## Pre-creation checklist

- [ ] Host conventions discovered or defaults declared
- [ ] Scope classified; prefix correct if scoped
- [ ] No duplicate in `<skills-root>/<skill-name>/` or index
- [ ] Frontmatter will contain only `name` and `description`
- [ ] `platform` and `script_policy` planned for Metadata block

---

## Post-creation report template

```
Created:
  <skills-root>/<skill-name>/SKILL.md
  [other paths]

Host conventions:
  skills root:    skills/
  skill file:     SKILL.md
  index:          [updated | skipped]
  audit-skill:    [pass | fail + output]
  audit-project:  [pass | fail + output | not run]

Incomplete:
  [items | none]
```
