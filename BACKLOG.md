# Backlog

Ideas for later. **Not** the active queue — see `TASKS.md` for committed work and
`docs/BUILD_PLAN.md` for phased gates. Capture possibilities here; move an item to `TASKS.md`
when you decide to schedule it.

Captured from architecture discussion (2026-05-24): compiled core, workspace boundaries,
GUI vs CLI hosts, clutter reduction.

---

## Near term (while canvas is still moving)

- [ ] **Host / canvas stability** — finish known layout bugs (sub-grid drag after profile load,
      sash snap, save/load round-trips). Decision context: `docs/decisions/007-canvas-layout.md`.
- [ ] **Archive `OLD-app/`** — legacy Go/Wails harness; no architectural role in current stack.
      Run `skills/audit-project/scripts/audit.ps1` after removal.

---

## Gaps & hygiene (identified, not yet scheduled)

- [ ] **Layout invariants doc** — captured in [`skills/canvas-layout/SKILL.md`](skills/canvas-layout/SKILL.md) + [REFERENCE.md](skills/canvas-layout/REFERENCE.md). Optional later: promote to `docs/decisions/008-canvas-layout-invariants.md`.
- [ ] **Re-save old mixed layout profiles** — slots saved before recent layout fixes may still
  hold bad `pixelLock` data; re-save after host stability work for clean round-trips.
- [x] **Canvas manual test checklist** — in [`skills/canvas-layout/REFERENCE.md`](skills/canvas-layout/REFERENCE.md).
- [ ] **Sync `docs/PROJECT_FEATURES.md` canvas section** — split-render / profile / sub-grid
      fixes may lag the feature record; update when host stability pass is done.
- [ ] **Sync `docs/PHASES.md`** — still oriented to Phase 2; TASKS shows Phase 3 mostly complete.
      Align quick-reference with `TASKS.md` so agents read the right “current phase.”

---

## Platform shape (when host feels stable)

- [ ] **Loader Phase 2 — less static wiring** — today extensions use real manifests + linter +
      dependency order, but each Rust extension still needs a factory registered in `lib.rs`
      (`loader.rs`: “Phase 1: static compilation”). Goal: add/change extensions without editing core
      bootstrap for every id.
- [ ] **Built-in vs user `extensions/`** — distinguish shipped extensions from a user-modifiable
      folder (or profile-driven enable list via `nulqor.toml`). Related: `enabled_extensions` in
      startup config today.
- [ ] **`host-cli` extension** — headless host mode (HTTP/MCP/scriptable) alongside GUI `host`.
      Core stays mode-agnostic; launch picks host extension (`--cli` / default GUI). Not the same as
      **Bundle mode** in `docs/GOAL.md` (frozen export vs live workspace).

---

## Distribution & workspace (later)

- [ ] **Core release artifact** — ship a prebuilt binary so day-to-day work can live in
      extensions + skills only. Core source stays available for core bugs, but not in every agent
      workspace.
- [ ] **Extension-only workspace / repo split** — optional `nulqor-core` releases + user project
      containing only `extensions/`, `skills/`, `rules/`, `nulqor.toml`. Biggest “clutter reduction”
      win for agents; binary alone does little without this split.
- [ ] **Bake v1 (frozen profile)** — Phase 5+ in `docs/BUILD_PLAN.md`: pin extension closure, versions,
      layout, skills into a reproducible profile. Standalone installer is a later step.
      Preconditions: `docs/DESIGN.md` §12 (static command/event refs — already enforced).

---

## Already planned elsewhere (not duplicated as tasks)

| Topic                                    | Where                                     |
| ---------------------------------------- | ----------------------------------------- |
| Active phase work                        | `TASKS.md`, `docs/PHASES.md`              |
| SQLite, `.nulqor`, agent loop            | Phase 4 in `docs/BUILD_PLAN.md`           |
| Bake/export, A/B compare, memory         | Phase 5+ in `docs/BUILD_PLAN.md`          |
| Canvas vs Bundle concept                 | `docs/GOAL.md` §179                       |
| Frozen core (8 responsibilities)         | `docs/DESIGN.md` §2, `docs/decisions/001` |
| “Dynamic load can wait” (Phase 1 choice) | `archive/product-brief-monolith.md`       |

---

## Decision rule

Promote an item to `TASKS.md` when you intend to work on it soon and can define “done.”
Until then, keep it here.
