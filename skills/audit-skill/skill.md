---
name: audit-skill
description: Audits skills for structural compliance, contract completeness, and index drift. Use before phase gates or after bulk skill changes.
---

## Metadata

```text
version:       2.0.0
topics:        meta, audit, skills, lint, compliance
platform:      all
script_policy: required
scope:         generic
```

Diagnostic counterpart to `create-skill`: prescriptive vs structural lint.
Format baseline: `skills/create-skill/SKILL.md`.

---

## When to use

- Before advancing past a phase gate.
- After creating or editing skills in bulk.
- When `skills/index.md` may be out of sync with disk.
- When a skill was edited outside the `create-skill` workflow.
- As a periodic health check on the skill tree.

Do not use for behavioral quality testing (see [REFERENCE.md](REFERENCE.md) evals).

---

## Contract

```text
when:         Before phase gates, after bulk skill creation, or when skills may
              have drifted from the create-skill format standard.
inputs:       none — reads skills/ directly; optional skill_name for one skill
outputs:      per-skill status PASS | WARN | FAIL | SKIP with findings
side-effects: none — read-only; no files modified
validation:   scripts/audit.ps1 exits 0 (no FAIL findings) for scope audited
```

---

## Steps

1. **Run the linter** — from repo root via `scripts/audit.ps1` or `scripts/audit.sh`:

   ```powershell
   powershell -ExecutionPolicy Bypass -File skills/audit-skill/scripts/audit.ps1
   ```

   Single skill: `-SkillName <name>` or `-SkillPath skills/<name>`. WARN/FAIL only: `-Quiet`. JSON: `-Json`.

2. **Read the report** — exit code 0 = no FAILs; exit code 1 = at least one FAIL.
   Check IDs: [REFERENCE.md](REFERENCE.md).

3. **Produce a fix plan** for each FAIL and WARN (do not fix in the same pass unless asked).

4. **Report** — totals, priority fix list, index drift if any.

---

## Verification

- [ ] `scripts/audit.ps1` ran for the intended scope.
- [ ] Every FAIL has a check ID and proposed fix.
- [ ] Script exit code recorded (0 = gate clear, 1 = FAILs present).
