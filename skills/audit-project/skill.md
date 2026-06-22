---
name: audit-project
description: Verify repo layout, extension colocation, registry sync, and run nulqor-lint. Use after file moves, restructures, or new extensions.
---

## Metadata

```text
skill_version: 1.0.0
applies_to:    nulqor
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
inputs:       root -- repo root (default .); skip_lint -- optional; strict -- optional
outputs:      pass | fail with FAIL: lines (hard violations) and WARN: lines (advisory)
side-effects: none — read-only
validation:   scripts/audit.ps1 exits 0; no FAIL lines in output (-Strict promotes WARN to FAIL)
```

---

## Steps

1. From repo root, run `scripts/audit.ps1` or `scripts/audit.sh`:

   ```powershell
   skills/audit-project/scripts/audit.ps1 [-Root <path>] [-Quiet] [-SkipLint] [-Strict]
   ```

2. Fix every `FAIL:` line. Re-run until exit code 0. Review `WARN:` lines; use `-Strict`
   (recommended in CI) to fail the build on them too.

**Hard checks (FAIL):** top-level dirs and indexes; forbidden legacy paths; extension colocation;
registry sync (disk ↔ `extensions/index.md` ↔ `mod.rs` ↔ `lib.rs`); `nulqor-lint`.

**Advisory checks (WARN, FAIL under -Strict):** doc drift (`PHASES.md` current phase vs `TASKS.md`;
core-responsibility count across `GOAL.md`/`DESIGN.md`/`decisions/001`); duplicate default ports
across extensions; polling sites (`setInterval`) that should prefer event push.

---

## Verification

- [ ] `audit.ps1` ran from intended repo root.
- [ ] Exit code 0 and no `FAIL:` lines.
- [ ] `WARN:` lines reviewed (doc drift, ports, polling) and addressed or consciously deferred.

Note: `scripts/audit.sh` does not yet mirror the advisory WARN checks (`-Strict`, doc drift, ports,
polling). Use `audit.ps1` for those until sh parity lands.
