# Nulqor — Current Phase Status

Quick-reference for building agents. Full spec in `BUILD_PLAN.md`.

---

## Current: Phase 4 — Persistence & harness essentials (in progress)

**Goal:** Durable sessions, project files, the agent loop, and a context manager — still extensions only.

| Task | Status |
|---|---|
| 4.1 Persistence extension — SQLite + FTS5 (index over `.nulqor/`, not a second truth store) | ⬜ Pending |
| 4.2 Project save/load — `.nulqor` files | 🟡 Partial (sessions v1: `sessions/*.jsonl` + `human/**`; room mode, search, CLI parity in BACKLOG) |
| 4.3 Agent-loop extension — plan→act→observe→verify (ships with the `DESIGN.md §13` iteration cap + its test) | ⬜ Pending |
| 4.4 Context manager — token budget + compaction | ⬜ Pending |
| 4.5 Decision-records workflow command | ⬜ Pending |

See `TASKS.md` Phase 4 and `docs/decisions/009-sessions-file-store.draft.md` before continuing.

---

## Outstanding proof: Phase 3 task 3.3 — the loop must be *shown* to close

Phases 0–2 gates passed; Phase 3 infrastructure shipped (skill-runner, validation, run-logger, the
`rules/current-date.md` temporal artifact). **The one thing not yet done is the thesis itself:**
task `3.3` — a human-driven Subject-model session demonstrating that one captured artifact turns a
repeatable failure into a repeatable success, and holds on a second related task.

Until `3.3` is run and documented, every Phase 4+ feature is built on an undemonstrated bet. Treat
`3.3` as the real exit gate for Phase 3.

**Known candidate (from `runs/2026-05-24.jsonl`):** Gemma 4 E4B fails temporal questions; fix is the
current-date rule already shipped — it only needs the before/after demo recorded.

---

## Gate history

- **Gate 0/1/2: ✅ PASSED** — `cargo test --workspace` green; HTTP API (default port 8787, override
  `NULQOR_PORT`) with full decisions/006 §1–3 surface; frozen core untouched.
- **Gate 3:** open — blocked on `3.3` (the human-driven loop-closure demo).

---

## Upcoming

- **Phase 5+:** A/B compare, bake/export, memory, linter UI, model routing

See `BUILD_PLAN.md` for full task lists and gates.
