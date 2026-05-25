# Nulqor — Agent Guide


## Boundaries

**NEVER**
- Overwrite operational files (docs, plans, skills) in place — draft first
- Execute `.draft` files
- Initialize or interact with git
- Improve, refactor, or restructure code outside the task scope
- Silently ignore failed commands, lint, or test output
- Re-derive steps that an existing script already handles — run the script

**ASK**
- Before adding a dependency to `Cargo.toml` or `package.json`
- Before a change touches more than one extension
- When requirements conflict with `docs/PHASES.md` or `docs/DESIGN.md`
- When ambiguity affects architecture, behavior, data shape, or scope — present options, don't pick silently

**ALWAYS**
- Read `docs/PHASES.md` and `TASKS.md` before starting non-trivial work
- Check `skills/index.md` — if a skill matches the task, read its `SKILL.md` first
- Read any file before editing it — never edit from assumptions
- State a brief plan before non-trivial edits; for trivial edits, proceed and state assumptions inline
- Remove only imports, variables, or files that your own changes made unused
- Update `docs/SPEC.md`, `DESIGN.md`, `PHASES.md`, or `decisions/` when the change affects them
- Run `skills/audit-skill/scripts/audit.ps1 -Quiet` after any skill change
- Run `skills/audit-project/scripts/audit.ps1 -Quiet` after any file move, rename, or restructure
- Report clearly when a command fails — never invent success
- Determine state before interacting — never assume OS, tool, app, or runtime versions; check first, then act
- When a tool call fails and a non-obvious workaround was required, flag it as a skill capture opportunity
- After completing any research or process that the user validates as reusable, propose running `create-skill`

## Where to look

- `docs/index.md` — project documentation (spec, design, phases, decisions)
- `extensions/index.md` — runtime extensions and their manifests
- `skills/index.md` — reusable agent workflows and scripts
- `TASKS.md` — active task queue and definition of done
- `README.md` — build commands, prerequisites, audit commands
