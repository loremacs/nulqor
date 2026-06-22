---
name: edit-and-verify
description: Edit repo code safely — read first, verify with tsc/cargo/audits, report failures. Use when changing Nulqor source code.
---

## Metadata

```text
skill_version: 1.0.0
applies_to:    nulqor
topics:        meta, edit, verify, test, workflow
platform:      all
script_policy: none
scope:         project-scoped
```

The `scripts/` directory is intentionally empty (instruction-only skill).

---

## When to use

- Changing Nulqor source (extensions, core, tools, skills, UI).
- Any task where success requires command output.

Load a narrower skill first when one applies (`create-extension`, `create-skill`, etc.).

---

## Contract

```text
when:         Before or after editing Nulqor repo source files
inputs:       task (what to change), scope (extension id or path area)
outputs:      files_changed list, verify_results pass/fail per command
side-effects: modifies only files required by the task
validation:   applicable verifiers run; failures reported with full output
```

---

## Steps

1. Read [`AGENTS.md`](../../AGENTS.md), [`README.md`](../../README.md), and target files.

2. Make a minimal diff. **If the change is OS-specific**, use platform guards (`AGENTS.md` § Multi-platform targeting) — do not break other OSes.

3. Run verifiers:

   ```powershell
   npx tsc --noEmit
   cargo check --workspace
   skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet
   skills/audit-project/scripts/audit.ps1 -Quiet
   ```

4. Report results — never claim success without output.

Stack truth: `package.json`, `Cargo.toml`, [`rules/stack-and-tooling.md`](../../rules/stack-and-tooling.md).

---

## Verification

- [ ] Target files read before edit.
- [ ] Verifiers ran; failures reported with full error text.
