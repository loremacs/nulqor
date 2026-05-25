# Reference — create-skill

Loaded on demand. Keep `SKILL.md` lean; read this for discovery defaults, scope rules,
description rules, progressive disclosure, and anti-patterns.

---

## Host defaults (Nulqor)

```text
skills root:     skills/
skill file:      SKILL.md
index:           skills/index.md — columns: Skill | Purpose
validator:       skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet
layout audit:    skills/audit-project/scripts/audit.ps1 -Quiet (after index or layout change)
scaffold:        skills/create-skill/scripts/create.ps1
agent entry:     AGENTS.md, README.md
```

State which defaults were used in the final report.

---

## L1 discovery: index vs parsing frontmatter

Two valid catalog patterns:

| Pattern | How L1 works | Tradeoff |
|---|---|---|
| Parse frontmatter | Runtime reads each `SKILL.md` up to closing `---` | Always in sync; N file touches |
| Maintained index | One `skills/index.md` row per skill | One read; must stay in sync with skills |

This host uses **maintained index** for cheap discovery. Frontmatter `name` and `description`
remain source of truth per skill; `skills/index.md` duplicates **Skill** (name) and
**Purpose** (one-line description) for agents that read one file first. Keep the index row
in sync manually — `audit-skill` validates frontmatter only; `audit-project` checks that
`skills/index.md` exists.

---

## Host convention discovery checklist

- [ ] Skills root path
- [ ] Skill filename (`SKILL.md`)
- [ ] Frontmatter policy (name + description only in YAML)
- [ ] Required body sections
- [ ] Index or catalog file
- [ ] Task tracker (optional — Nulqor uses root `TASKS.md` for human queue, not per-skill backlog)
- [ ] Validator command
- [ ] Script / platform policy

---

## Scope classification and prefix table

| Scope | Definition | Prefix example |
|---|---|---|
| generic | Portable; no host-specific paths or product names | none |
| os-scoped | One operating system | `win-`, `mac-`, `linux-` |
| tool-scoped | Public tool with external docs | `git-`, `docker-`, `github-` |
| domain-scoped | Framework or domain | `rust-`, `python-`, `node-` |
| project-scoped | One org/repo only | clear external prefix |

Prefixes must be understandable outside this project. Do not use a team-only app name
as a prefix unless the skill is intentionally project-scoped.

Scoped skills may reference their tool/OS freely. Generic skills read project specifics
from the host's docs at runtime — do not hardcode them in the skill body.

---

## Description rules

Operational field — drives index rows and semantic routing.

**Must:** what it does + when to trigger + indirect phrasings agents might use.

**Must not:** "always use", "best", "critical", "mandatory for all tasks", unrelated keywords.

**Security:** padded descriptions can bias skill selection (supply-chain risk). Keep honest and specific.

---

## Frontmatter vs Metadata

**Frontmatter (YAML, before first body content):**

```text
name          required — matches directory name
description   required — what + when
```

Optional *standard* fields only if the host runtime requires them:
`license`, `compatibility`, `metadata`, `allowed-tools`.

**Do not put version, topics, platform, script_policy, or scope in frontmatter.**

**## Metadata (first body section, after closing ---):**

```text
version:       semver, start 1.0.0
topics:        comma-separated discovery tags
platform:      all | windows | macos | linux | combos
script_policy: none | optional | required
scope:         generic | os-scoped | tool-scoped | domain-scoped | project-scoped
```

Parsers that stop at the second `---` load only L1 (`name` + `description`).
Full body load (L2) includes Metadata and execution sections.

---

## Required sections

```text
frontmatter (name, description only)
## Metadata
## When to use
## Contract
## Steps
## Verification
```

Optional body sections: `## Requirements`, `## Known failures`, `## Revision notes`.
Optional files: `FORMS.md`, `REFERENCE.md`, `EXAMPLES.md`, `scripts/`, `references/`, `assets/`, `data/`.

---

## Progressive disclosure

| Stage | Loads | Budget |
|---|---|---|
| L1 Catalog | `name` + `description` (or index row) | ~100 tokens/skill |
| L2 Activation | Full `SKILL.md` body after closing `---` | < 500 lines / < 5000 tokens |
| L3 On demand | `REFERENCE.md`, scripts, other siblings | when a step references them |

There is no official "load half the body" stage — L2 is the entire markdown body at once.

---

## Scope rule (body content)

Every line in `SKILL.md` must serve one of:

1. Execute the contract (steps, contract, verification).
2. Non-derivable reference the agent cannot infer (Metadata block, short context).
3. A verification checklist item.

Remove lines that only explain what the skill does *not* cover or point elsewhere without
being a required step. Anti-patterns belong in `REFERENCE.md` or host docs unless they
prevent a known execution failure for this specific skill.

---

## Script policy

```text
none      instruction-only
optional  scripts allowed
required  scripts are part of execution
```

- `platform: all` + scripts → `.ps1` + `.sh`, or one cross-platform script (Python, Node, Go).
- `platform: windows` → `.ps1`
- `platform: macos` / `linux` → `.sh`
- Missing companion → report incomplete work in final output; do not claim the skill is complete.

---

## Anti-patterns

- Host metadata in frontmatter instead of `## Metadata`.
- Using `skill.md` when host standard is `SKILL.md`.
- Creating a skill without checking host conventions or index for duplicates.
- Description without a trigger condition or with trigger bait.
- `platform: all` with only one platform script and no incomplete-work note in the report.
- Omitting `validation:` in Contract.
- Reporting success without validator result.
- Body over 500 lines without moving detail to `REFERENCE.md`.
- Repeating Metadata fields inside `## Contract`.

---

## Revision rule

When a skill fails in use: tighten description, shorten steps, add validation, or move
detail to reference files. Bump `version` in `## Metadata`. Log significant changes in
`## Revision notes` if the skill already has that section.
