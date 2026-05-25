---
name: create-extension
description: Scaffold extensions/<id>/ with colocated manifest, Rust, and optional panel UI. Use before adding any new extension.
---

## Metadata

```text
version:       1.0.0
topics:        meta, extensions, scaffold, tauri
platform:      all
script_policy: required
scope:         project-scoped
```

Never hand-create extension folders. Always run `scripts/create.ps1` first.

---

## When to use

- Adding a new Nulqor extension (Service, Panel, Host, or Provider).
- Asked to scaffold extension layout under `extensions/<id>/`.

Do not use for skills — use `create-skill`.

---

## Contract

```text
when:         Before creating any new extension directory
inputs:       id (kebab-case), kind, purpose, optional requires (comma-separated ids)
outputs:      extensions/<id>/ tree; mod.rs bridge; index row; loader.register hint
side-effects: creates files; updates extensions/index.md and mod.rs
validation:   audit-project passes; loader.register added to lib.rs
```

---

## Steps

1. Run `scripts/create.ps1` or `scripts/create.sh`:

   ```powershell
   skills/create-extension/scripts/create.ps1 -Id my-extension -Kind Service -Purpose "..."
   ```

2. Add `loader.register(...)` to `src-tauri/src/lib.rs`.

3. Implement `extensions/<id>/src/lib.rs` and optional `ui/`.

4. Run `skills/audit-project/scripts/audit.ps1 -Quiet` and `cargo test --workspace`.

---

## Verification

- [ ] `extensions/<id>/` exists with manifest, README, `src/lib.rs`.
- [ ] `audit-project` exit code 0.
