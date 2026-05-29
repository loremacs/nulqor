# Backlog

Ideas for later. **Not** the active queue — see `TASKS.md` for committed work and
`docs/BUILD_PLAN.md` for phased gates. Capture possibilities here; move an item to `TASKS.md`
when you decide to schedule it.

**New session — chat / sessions / group chat:** read [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md) first (v1 shipped vs designed gaps). Shipped behaviour: `docs/PROJECT_FEATURES.md` §2.4–2.4b. Checkbox backlog: **Chat, sessions & group chat** below.

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
- [ ] **Sync `docs/PROJECT_FEATURES.md` canvas section** — window mode + click-through rules updated (2026-05); split-render / profile items may still lag. Optional: promote window-mode rules to `docs/decisions/008-window-mode-ui.draft.md`.
- [x] **Session / chat UI decision** — [`docs/decisions/009-sessions-file-store.draft.md`](docs/decisions/009-sessions-file-store.draft.md) (canonical handoff). Checkbox backlog: **Chat, sessions & group chat** below.
- [ ] **Sync `docs/PHASES.md`** — still oriented to Phase 2; TASKS shows Phase 3 mostly complete.
      Align quick-reference with `TASKS.md` so agents read the right “current phase.”

---

## Chat, sessions & group chat (designed — not fully built)

Captured from architecture discussion (2026-05). **v1 shipped:** `session-store` + `chat-panel` (file persistence, rail, fork-on-edit, overlay). Everything below is **not implemented** or only partially so.

### Storage & catalog

- [ ] **`human/catalog/INDEX.md`** — human-readable session index with title, date, summary links (today: `catalog.json` only).
- [ ] **Session frontmatter / metadata header** — per-session `mode`, `title`, `participants`, `branch_policy` in file header (today: jsonl lines only + catalog fields).
- [ ] **Project-scoped vs global sessions** — `.nulqor/` under project root vs user home; not decided in config.
- [ ] **Import `runs/*.jsonl`** into reopenable sessions (or keep runs audit-only).
- [ ] **SQLite + FTS indexer** (Phase 4) — deep search over same files; not a second truth store.

### Session modes (thread vs room / group)

- [ ] **`mode: thread`** (default) — linear assistant context; edit truncates or forks active branch.
- [ ] **`mode: room`** — Discord/Slack-style group: many humans + many agents; **in-place edits** (no truncate/regenerate); chronological shared log; no fork-on-edit.
- [ ] **Participant roster** — explicit `participants[]` on session (today: per-message `participant_name` only).
- [ ] **Multi-agent interleave** — room mode agents reply without single `active_agent` monopoly (design TBD).

### Thread mode — branches & agent isolation

- [ ] **`branch_policy: preserve`** (default) vs `truncate` — preserve = snapshot old tail to `human/branches/` on edit (partially built); truncate = discard tail with no archive.
- [ ] **Agent must not read archived branches** — enforce in generate/MCP paths (design rule; not fully gated — `human/**` not loaded but no explicit capability wall).
- [ ] **Optional override** — one-shot `include_archived_branches` on generate; logged; non-default.
- [ ] **Restore branch as active** — explicit human action (would fork again); not built.
- [ ] **True branching UI** — multiple live branches (today: one active + archived snapshots only).

### Human rail (conversation map)

- [ ] **Rail is human-only** — never exposed to MCP/agent commands (partially true; needs capability/permission gate).
- [ ] **Main chat shows no fork chrome** — fork access only via rail (v1: rail fork row + overlay; OK).
- [ ] **Symbol picker** — ★ ⚑ ? 💡 presets for user bookmarks (v1: partial via Mark dropdown).
- [ ] **POI / verbose markers** — flag long reasoning or tool dumps on rail without cluttering main chat.
- [ ] **Filters** — All | Human | Agents | Bookmarks | Forks.
- [ ] **Optional ghost archived branch** in rail (dimmed timeline); not built.
- [ ] **Regenerate INDEX / summaries** from session headers on list.

### Message payload & extras

- [ ] **`payload` object** on messages — `reasoning`, tool calls/results, system prompt hash, token stats (today: top-level `reasoning` only).
- [ ] **Frozen payload in archived forks** — full audit for human inspection.

### Surfaces & parity

- [ ] **`cli-panel`** — `session list`, `session open`, `rail list/jump`, `branch show` (same commands as UI).
- [ ] **Chat streaming** — live token stream via Tauri events (today: 2s poll).
- [ ] **Session search** — by title, date, marker, FTS.
- [ ] **Room mode UI** — roster, per-speaker bubbles, edit-in-place UX.

### Decision doc

- [x] **`docs/decisions/009-sessions-file-store.draft.md`** — canonical spec + handoff for next session.

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
| Host window mode + click-through UI | `docs/PROJECT_FEATURES.md` §0.7, `skills/canvas-layout/REFERENCE.md` |
| Chat/session v1 (implemented) | `docs/PROJECT_FEATURES.md` §2.4–2.4b, `extensions/session-store/README.md` |
| Chat/session/group (design + gaps) | `docs/decisions/009-sessions-file-store.draft.md`, `BACKLOG.md` § Chat |
| Frozen core (8 responsibilities)         | `docs/DESIGN.md` §2, `docs/decisions/001` |
| “Dynamic load can wait” (Phase 1 choice) | `archive/product-brief-monolith.md`       |

---

## Decision rule

Promote an item to `TASKS.md` when you intend to work on it soon and can define “done.”
Until then, keep it here.
