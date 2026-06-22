# Reference — audit-skill

Loaded on demand. Checklist, report format, security scan rules, and behavioral evals.

---

## Structural vs behavioral

| Type | What it checks | How |
|---|---|---|
| Structural (this skill) | Files, frontmatter, Metadata, Contract, index | `scripts/audit.ps1` |
| Behavioral (optional) | Instructions produce correct output | `evals/` + agent runner |

Run structural audits frequently. Run behavioral evals before marking a skill stable.

---

## Status levels

```text
PASS    required checks pass (F, M, MD, C, I, P, S1)
WARN    required pass; one or more Q, P, C6, or S2 findings
FAIL    one or more F, M, MD, C, I, or S1 failures
SKIP    explicitly excluded from audit (list in audit.ps1 $SKIP_SKILLS; usually empty)
```

FAIL blocks a phase gate. S1 FAIL = urgent (dangerous script pattern).

---

## Audit checklist (per skill)

### F — File structure

- **F1** `SKILL.md` exists (`skill.md` only = FAIL, rename required)
- **F2** `scripts/` directory exists (may be empty)
- **F3** No files outside `SKILL.md`, `scripts/`, `references/`, `assets/`, `evals/`, `data/`, optional `FORMS.md`, `REFERENCE.md`, `EXAMPLES.md`
- **F4** No unexpected nested directories

### M — Frontmatter (L1: `name` + `description` only)

- **M1** Opens with `---` on line 1
- **M2** `name:` present
- **M3** `name` matches directory name (case-sensitive)
- **M4** Lowercase, hyphens, no consecutive hyphens
- **M5** `name` ≤ 64 characters
- **M6** `description:` present and non-empty
- **M7** WARN if description lacks what + when trigger
- **M12** WARN if `version`, `topics`, or `platform` in frontmatter (belong in `## Metadata`)
- **MF1** Closing `---` within first 30 lines

### MD — Metadata section

- **MD1** `## Metadata` exists
- **M8** `skill_version:` present and valid semver (legacy `version:` = FAIL, rename required)
- **MD2** `applies_to:` present and non-empty (e.g. `nulqor`, `tauri@2`)
- **M9** `topics:` present and non-empty
- **M10** `platform:` present and valid
- **M11** OS prefix (`win-`, `mac-`, `linux-`) matches `platform`

### C — Contract

- **C1** `## Contract` heading exists
- **C2** Code block immediately after Contract
- **C3** `when:` present
- **C4** `outputs:` present
- **C5** `side-effects:` present
- **C6** WARN if Contract repeats `name:`, `version:`, or `topics:`
- **C7** `validation:` present

### P — Platform and scripts

- **P1** `platform: all` + `.ps1` in `scripts/` → WARN without `.sh` or cross-platform script (report incomplete work if blocked)
- **P2** `platform: all` + `.sh` → WARN without `.ps1` or cross-platform script

### I — Index (Nulqor)

- **I1** Row in `skills/index.md` for skill name
- **I2** Index row has ≥2 columns (`Skill | Purpose`)

### Q — Quality (WARN)

- **Q1** `description` under 200 characters (recommendation)
- **Q2** Body under 500 lines
- **Q3** `## Verification` with checklist items
- **Q4** `## When to use` (or legacy `## Problem`)
- **Q5** Scripts in `scripts/` referenced in body
- **Q6** Body matches declared `scope` in Metadata
- **Q7** No scope-padding; every line serves contract, Metadata, or verification

### S — Security (scripts only)

Comment lines and `# nosec` suffixes are skipped during script security scan.

**S1 FAIL (CRITICAL):** `iex`/`Invoke-Expression` with variable or pipe; `curl|bash`, `wget|sh`, `iwr|iex`; Base64 decode into execution; `eval` with variable input.

**S2 WARN (HIGH):** External HTTP; destructive `Remove-Item -Recurse` / `rm -rf` outside repo; `.ssh/`; system dir writes; registry writes; `Set-ExecutionPolicy`; `Disable-*` security cmdlets.

---

## Report format

```text
SKILL AUDIT REPORT — <date>
===========================

skills/<name>                    WARN
  [Q3] no ## Verification section

skills/create-skill              PASS

SUMMARY
  Total / PASS / WARN / FAIL / SKIP

PRIORITY FIXES
  1. FAIL — before phase gate
  2. WARN — before marking skill stable
  3. Index drift — append missing rows
```

Do not maintain a static baseline table in `SKILL.md`; the script is source of truth.

---

## Script usage

```powershell
# All skills
powershell -ExecutionPolicy Bypass -File skills/audit-skill/scripts/audit.ps1

# One skill
powershell -ExecutionPolicy Bypass -File skills/audit-skill/scripts/audit.ps1 -SkillName create-skill

# Legacy alias (skill directory path)
powershell -ExecutionPolicy Bypass -File skills/audit-skill/scripts/audit.ps1 -SkillPath skills/create-skill

# WARN/FAIL only
powershell -ExecutionPolicy Bypass -File skills/audit-skill/scripts/audit.ps1 -Quiet

# JSON
powershell -ExecutionPolicy Bypass -File skills/audit-skill/scripts/audit.ps1 -Json
```

**CRLF:** The script normalizes `\r` before regex matches (required on Windows).

---

## Common WARNs

- **Q5** — Instruction-only skills: add one line that `scripts/` is intentionally empty.
- **P1/P2** — Missing companion script: report incomplete work in the audit fix plan.
- **M7** — Multiline YAML `description:` may not parse trigger words; use `description: >` with explicit "Use when".

---

## Behavioral evals (optional)

```text
skill-name/
├── SKILL.md
├── scripts/
└── evals/
    ├── evals.json
    └── files/
```

Add `evals/` when the skill is high-traffic, recently changed, or approaching stable `1.x`.
Requires an agent runner (manual in Cursor/Claude until Phase 4).

Example `evals.json` shape: see https://openagentskills.dev/docs/specification or host docs.
