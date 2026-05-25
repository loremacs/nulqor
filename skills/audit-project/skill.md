---
name: audit-project
description: Verify repo layout, extension colocation, registry sync, and run nulqor-lint. Use after file moves, restructures, or new extensions.
---

## Metadata

```text
version:       1.0.0
topics:        meta, audit, layout, extensions, lint
platform:      all
script_policy: required
scope:         project-scoped
```

Layout integrity audit for the Nulqor repo. Companion to `audit-skill` (skills tree only).

---

## When to use

- After any file move, rename, or directory restructure.
- After scaffolding or editing an extension.
- Before merging layout-sensitive changes.

Do not use for skill-format lint — run `audit-skill` instead.

---

## Contract

```text
when:         After layout, extension, or index changes in the repo
inputs:       root -- repo root (default .); skip_lint -- optional
outputs:      pass | fail with FAIL: lines listing violations
side-effects: none — read-only
validation:   scripts/audit.ps1 exits 0; no FAIL lines in output
```

---

## Steps

1. From repo root, run `scripts/audit.ps1` or `scripts/audit.sh`:

   ```powershell
   skills/audit-project/scripts/audit.ps1 [-Root <path>] [-Quiet] [-SkipLint]
   ```

2. Fix every `FAIL:` line. Re-run until exit code 0.

**Checks:** top-level dirs and indexes; forbidden legacy paths; extension colocation;
registry sync (disk ↔ `extensions/index.md` ↔ `mod.rs` ↔ `lib.rs`); `nulqor-lint`.

---

## Verification

- [ ] `audit.ps1` ran from intended repo root.
- [ ] Exit code 0 and no `FAIL:` lines.
