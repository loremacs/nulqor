# Opus 4.8 — Skills Logic & Process/Goal Evaluation

Companion to [`OPUS-4.8-REVIEW.md`](OPUS-4.8-REVIEW.md). Covers: is the skills system coherent, and
is the process/goal well thought out and self-consistent.

---

## 1. Skills system — is the logic sound?

Yes, structurally. The skills layer is one of the better-designed parts of the repo:

- **Discovery is cheap by design.** `skills/index.md` is a compact scan table; full bodies load on
  demand via `skill-runner:load@1` → `context-editor:load-skill@1`. This directly serves the 8 GB
  token-budget constraint (`DESIGN.md §15-16`): the model sees a short index, not every skill body.
- **Skills are self-bootstrapping.** `create-skill` and `create-extension` scaffold new artifacts and
  register them in the relevant index, and `audit-skill`/`audit-project` mechanically enforce layout,
  frontmatter, and index registration. This is the compounding loop applied to the toolchain itself —
  exactly what `GOAL.md` says should happen ("the first user of the tool is the tool itself").
- **Execution is logged.** `skill-runner` traces every invocation, satisfying the `DESIGN.md §13`
  "every skill execution logged" gate, and a missing skill returns `{ found: false }` rather than
  throwing — honest failure over silent failure (operating principle #2).

The seven shipped skills (`audit-skill`, `audit-project`, `create-extension`, `create-skill`,
`edit-and-verify`, `canvas-layout`, `nulqor-communicate`) are all *process* artifacts — they make the
build reproducible. That is the right first set.

### Gaps in the skills logic

- **No skill yet captures a Subject-model task.** Every current skill is about *building Nulqor*, not
  about *driving a bounded task through the Subject model and capturing the fix*. That is the loop the
  thesis is actually about (`GOAL.md` compounding loop), and there is no skill exercising it. The
  skills system is proven on the meta-task (building the platform) but not on the target task
  (constraining a small model). Until a Subject-task skill exists and demonstrably removes future work,
  the skills layer proves self-hosting, not the central bet.
- **`matches_regex` weakens skill-driven validation.** Skills that lean on `validation:check@1` for a
  pass/fail gate inherit the misrepresented regex check (see `OPUS-4.8-TESTS.md §2`). Any skill that
  validates with a regex pattern is silently doing substring matching.
- **Index format is documented in two places.** `skills/index.md` points to
  `decisions/006 §7` for frontmatter rules while `create-skill/references/skill-format.md` also defines
  the format. Single-source it to avoid future drift.

---

## 2. Is the process well thought out?

The *shape* of the process is excellent and rare. `GOAL.md` explicitly enumerates its own death modes
(core creep, silent learning, asking the Subject to build, context bloat, over-planning) and maps each
to a guard. `DESIGN.md` resolves the four hard seams (contract coexistence, named capability instances,
event scope, concurrency) up front. The decision records (`decisions/001-006`, `009 draft`) preserve
*why*, not just *what*. This is materially more disciplined than typical projects of this size.

Where the process is **not** yet living up to its own design:

### 2a. The loop has never closed
`GOAL.md` is unambiguous: loop-closure "should be demonstrated as early as possible," and it is the
"proof the thesis holds." `TASKS.md` task `3.3` is still `⬜ Pending`. The process documents a
compounding loop in detail and has built all the infrastructure for it (skill-runner, validation,
run-logger, the temporal artifact), but has never run one full iteration end to end. The process is
correct on paper and unexecuted in practice — the single most important discipline in the project
(per `GOAL.md`: "iteration without capture is not compounding — it is just churn") has not been tested
even once.

### 2b. Self-inconsistency in the knowledge base
The product *is* durable, inspectable process knowledge — so internal contradictions are not cosmetic,
they undercut the value proposition:

- `docs/PHASES.md` says "Current: Phase 2"; `TASKS.md` shows Phase 3 mostly done and early Phase 4
  work shipped (session file store, workbench), though SQLite/FTS (task 4.1) is still pending.
  `BACKLOG.md` itself flags this drift and it remains.
- `GOAL.md` failure-mode #1 still says "the five-responsibility core list is fixed," while
  `decisions/001` is literally titled "The core is frozen at **eight** responsibilities" and
  `DESIGN.md §2` lists eight. The guard against core creep documents a core that already crept, and
  was not updated.

These are small edits, but the audit tooling that enforces layout does **not** enforce *semantic*
consistency between the planning docs. Recommend extending `audit-project` to catch exactly this
(see `OPUS-4.8-TESTS.md §4` item 10).

### 2c. Breadth is outrunning proof
The process principle "real tasks create real knowledge (not speculative planning)" is in tension with
what shipped: three provider backends, a five-profile canvas layout engine, and designed-but-unbuilt
thread/room chat modes (`BACKLOG.md`, `decisions/009 draft`). Much of this is capability the proven
loop does not yet need. The canvas layout subsystem (`PROJECT_FEATURES.md §0.7`, `decision 007`) is
large and complex — in fairness it is genuine core product UX (a multi-panel shell), not throwaway
polish — but that complexity still competes for time against proving `3.3`.

---

## 3. Is the goal well thought out?

The goal itself is the strongest asset. It is specific, falsifiable, and honest about what it is *not*
(`GOAL.md` "What this is NOT"). The success criteria are measurable (loop closure, self-hosting,
8 GB budget). The bet — value in the harness, not the model — ages well regardless of model progress.

The one structural critique: the goal's success is defined by an event (`3.3`) that the build order
keeps deferring. A goal this clear deserves to be *tested* early, not approached asymptotically. The
process would be healthier if the build plan gated Phase 4 behind a demonstrated `3.3`, so that no
further breadth ships until the central claim is proven once.

---

## 4. Recommendations

1. **Author one Subject-task skill** that drives a bounded task through the Subject model, validates
   deterministically, and captures the fix — then prove it removes work on a second task. This is the
   missing center of the skills system.
2. **Single-source the skill format** (`decisions/006 §7` vs `create-skill/references/skill-format.md`).
3. **Make `audit-project` enforce semantic doc consistency** (current phase, core-responsibility count)
   so the knowledge base cannot silently contradict itself again.
4. **Gate further breadth behind `3.3`.** Treat loop-closure as the exit gate for Phase 3 in practice,
   not just on paper, before more Phase 4/5 surface area lands.
5. **Fix `matches_regex`** so skill-driven validation does not inherit a check that lies.
