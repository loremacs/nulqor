# Examples — create-skill

Worked examples. All paths use `SKILL.md`.

Example directory trees use `<placeholder>` paths (e.g. `/<skills-root>/<skill-name>/`)
so project ref scans do not treat them as real repo paths.

---

## Example 1 — Generic, no scripts

```
skill_name:     review-pull-request
description:    Reviews a PR for correctness and tests. Use when asked to
                "review this PR", "check the diff", or "give feedback on the PR".
topics:         process, git, review
platform:       all
script_policy:  none
scope:          generic
```

```
/<skills-root>/review-pull-request/
└── SKILL.md
```

---

## Example 2 — Windows-scoped with script

```
skill_name:     win-clear-rust-cache
platform:       windows
script_policy:  required
scope:          os-scoped
```

```
/<skills-root>/win-clear-rust-cache/
├── SKILL.md
└── scripts/
    └── clear-cache.ps1
```

No `.sh` companion — `platform: windows` only.

---

## Example 3 — Tool-scoped, platform all, two scripts

```
skill_name:     docker-build-image
platform:       all
script_policy:  optional
scope:          tool-scoped
```

```
/<skills-root>/docker-build-image/
├── SKILL.md
├── REFERENCE.md
└── scripts/
    ├── build.sh
    └── build.ps1
```

---

## Example 4 — Cross-platform Python (no backlog)

```
/<skills-root>/<skill-name>/
├── SKILL.md
└── scripts/
    └── audit.py
```

Python satisfies `platform: all` without a `.sh`/`.ps1` pair.

If only `audit.ps1` existed for a `platform: all` skill, report incomplete work:

```markdown
Incomplete: add .sh companion for /<skills-root>/<skill-name>/scripts/
            Platform: macos, linux. Blocked until testable on target platform.
```

---

## Example 5 — Index row (Nulqor)

`skills/index.md` uses two columns:

```markdown
| Skill | Purpose |
|---|---|
| `my-skill` | One-line what + when (matches frontmatter description) |
```

Insert under **Available Skills**, not at EOF. `skills/create-skill/scripts/create.ps1` does this automatically.
