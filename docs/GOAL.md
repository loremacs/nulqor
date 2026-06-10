# Nulqor — Goal

This file owns: **why** Nulqor exists, what the central bet is, what success and failure look like.
It owns the problem space, principles, roles, and non-goals (folded in from the former SPEC).
It does NOT own: how it is built (→ `DESIGN.md`) or the build order (→ `BUILD_PLAN.md`).

This document exists because a builder agent that does not understand the *why* will make locally
reasonable choices that are globally wrong. Read it before forming opinions about scope.

---

## The central bet

Nulqor is **not** a bet that we can make small models smart. That bet is mostly out of our hands —
it depends on the model, and any gain could vanish with the next release.

Nulqor is a bet that **constraint beats capability for bounded work**: if you make a task small
enough, give the model exactly the right context and tools, and validate the result
deterministically, then a limited model can reliably succeed — and the *apparatus* that makes this
possible is a durable, compounding asset.

Two consequences follow, and they shape everything:

1. **The value lives in the harness, not the model.** Models are interchangeable tenants passing
   through. The harness (steps, context discipline, tools, validation, captured artifacts) is what
   appreciates over time. A good harness helps a frontier model about as much as a small one —
   and that symmetry is the *proof* the value is in the right place, not in papering over a weak model.

2. **The first user of the tool is the tool itself.** Nulqor is built using Nulqor's own
   principles, and ultimately inside Nulqor. Every time the Builder struggles to build a Nulqor
   extension, that struggle is data: the fix is a tighter step or a new skill, which is the
   compounding loop running on Nulqor's own development. This is the fastest, cheapest feedback
   loop available, and it de-risks the whole thesis for free.

---

## The platform (what gets built on Nulqor)

Nulqor is an extensible building platform. The AI-harness capability — watching a Subject model,
improving skills, capturing artifacts — is the first class of app built on it because it proves
the thesis cheapest and provides the tightest feedback loop. It is not the only class.

Extensions can deliver any combination of panels, dashboards, data pipelines, home automation
surfaces, custom agent loops, domain-specific builders, integration tools, and background services.
Any canvas configuration that becomes stable can be **compiled into a standalone Bundle** — a
focused, deployable app that runs outside the IDE and requires no knowledge of the rest of the
platform. Apps that benefit from live connection to the builder (iteration-heavy tools, agent
workflows) stay in Canvas mode and are accessible from within IDE tools over HTTP/MCP.

**Extensions are cognitive scope boundaries for building agents, not just technical isolation.**
An IDE agent (Cursor, Windsurf) tasked with building or extending a feature needs context only
about that extension and the core API. It does not need to understand how other extensions work,
access their files, or hold the entire system in mind. This scoping is intentional: it is what
makes agent-assisted building reliable at scale. A bounded task is a tractable task.

**IDE tools are first-class building agents on this platform.** Cursor and Windsurf can author
new extensions, edit existing ones, test the Subject model against new skills, validate results,
and propose changes — all from within the IDE, over the HTTP API or MCP, with the human approving
anything persistent. The platform is built this way so that building it and using it are the
same activity.

---

## What "gets better over time" actually means

It does NOT mean the model gets better. It means: the *number of tasks the harness can reliably
drive a given model through* goes up, and the *effort to add the next one* goes down — because
prior work left behind reusable artifacts.

The improvement is measured in **captured artifacts that remove future work**, not in model
benchmark points. A few percent of model gain is a bonus, never the load-bearing claim.

## The compounding loop (the thing that must actually work)

```
Bounded task given to the Subject model
   ↓
Subject succeeds or fails (observed by Builder + Human in shared transcript)
   ↓
If it struggled: Builder tightens the step / adds a skill / adds context / adds a validation rule
   ↓
That fix is CAPTURED as a durable artifact (skill, script, rule, test, decision, template)
   ↓
Next task is easier because the artifact exists
   ↓
(repeat — the workbench is now measurably more useful than before)
```

**The loop only counts as closed if each iteration leaves behind an artifact that demonstrably
removes future work.** Iteration without capture is not compounding — it is just churn. This is the
single most important discipline in the project.

---

## Success criteria (how we know it is working)

- **Phase 1 success:** A human and an IDE agent can hold one shared conversation with the Subject
  model in a single window. (ALREADY DEMONSTRATED — protect it.)
- **Loop-closure success:** Take a bounded task the Subject *fails*. The Builder adds one artifact
  (skill/context/rule). The Subject then *passes*. The artifact persists and helps a second related
  task without rework. **This is the proof the thesis holds and should be demonstrated as early as
  possible.**
- **Self-hosting success:** A new Nulqor extension can be authored faster because of skills/templates
  captured while building previous extensions.
- **Budget success:** All of the above stays functional within the 8 GB VRAM whole-system baseline
  (see `DESIGN.md`). Full function at baseline is the requirement, not a degraded mode.

## Failure modes to actively guard against

These are the ways this project dies. Each maps to a rule or a metric.

1. **Core creep.** "Just this one useful thing in the core." The core stops being frozen; every
   change risks everything. → Guard: the eight-responsibility core list is fixed; additions need an ADR.
2. **Silent learning.** A fix lives in chat history, not an artifact. The same problem is solved
   twice. The loop quietly stops compounding. → Guard: capture discipline; watch for fixing anything twice.
3. **Asking the Subject to build.** Routing platform-authoring tasks to the small model because it
   is "the local one." It will fail, and the loop never starts. → Guard: the Builder builds; the
   Subject is observed. Never confuse them.
4. **Context bloat.** Fixed harness overhead eats the small model's working budget before any task
   content loads. → Guard: measure system-prompt+rules+skill-index token cost continuously; it is a
   budget line item, not free.
5. **Over-planning the unknowable.** Trying to fully specify a process you are still learning. →
   Guard: spec the *shape* (frozen core + versioned extensions), not the future features. Be cheap
   to be wrong.

---

## The problem space (why this is worth building)

Building software, tools, automations, interfaces, agents, and dashboards involves more than producing
code. A project also develops a *process*: how it is planned, what rules guide the work, what build
order is safest, what validation is required, what must never change without review, what repeated
steps could become scripts, what repeated patterns could become templates, what repeated failures
could become tests or rules, and what knowledge should survive beyond one session.

Today that process knowledge is trapped — in one conversation, one session, one person's memory. When
it is lost, the next project relearns it from scratch. Nulqor exists to make that process **durable,
reusable, inspectable, and compounding** across future work. The goal is not only to help produce a
project; it is to preserve and improve *the way projects are produced*.

A useful project should leave behind more than an output. It may also leave behind reusable process
artifacts: skills, extensions, scripts, templates, rules, tests, validation checks, decision records,
guides, project layouts, reusable panels, command definitions, event patterns. The next project
inherits the useful parts of the previous one. That inheritance is the compounding loop.

## Operating principles (the tie-breakers when choices conflict)

1. Reliability over capability.
2. Explicit failure over silent failure.
3. Small context over broad context.
4. Auditability for meaningful actions.
5. Confirmation before destructive or persistent changes.
6. Simplicity over cleverness.
7. Local ownership by default.
8. Open, parseable files where practical.
9. Real tasks create real knowledge (not speculative planning).
10. Empty stubs are honest when a feature is not built yet.

## The roles of the artifacts (what each kind of thing is FOR)

- **Extensions** are the primary way Nulqor grows, and the primary unit of app building on the
  platform. An extension may provide a panel, command set, service, provider, theme, tool, agent
  integration, script runner, file interface, automation surface, domain-specific builder, or bake
  workflow. Product behavior lives in extensions wherever practical; the core stays small. Each
  extension is also an **explicit cognitive scope boundary for building agents**: an agent tasked
  with an extension needs context only about that extension and the core API — not other extensions.
- **Skills** are reusable process artifacts that describe how to perform or validate a task. A useful
  skill helps a human or agent do a task more consistently than from memory alone, and reduces repeated
  explanation. (Format and execution rules: `DESIGN.md`.)
- **Scripts and validation** turn repeatable mechanical work into something deterministic. A script,
  test, or linter rule is preferable to repeatedly asking an agent to reason through the same checklist.
- **Decision records** preserve *why* a meaningful choice was made, its consequences, and its
  constraints — so reasoning is not lost across sessions. (`decisions/`.)
- **The agent is optional.** It may act as planner, document-router, editor, skill author, test
  assistant, refactor assistant, or reviewer — and may be part of a finished tool. The architecture
  supports agents without assuming every project needs one; Nulqor stays useful when a human builds
  manually with scripts, extensions, templates, and structure.

## Two operating modes (Canvas and Bundle)

- **Canvas mode — the live workspace.** Extensions are live, modifiable, testable, composable.
  Panels, scripts, skills, agents, commands, and project state are active and interactive while
  building. IDE tools like Cursor and Windsurf connect to the running canvas over HTTP/MCP and can
  author, test, and iterate without leaving the IDE. This is where apps are built and refined, and
  where small model behavior is observed and improved.
- **Bundle mode — the compiled output.** When a canvas configuration becomes stable enough to ship
  or reuse independently, a bake workflow compiles a *selected subset* of extensions and config into
  a **focused standalone app** — no IDE required, no full platform context, just the selected
  capability running on its own. (Bake-readiness constraint: `DESIGN.md §12`.)

The building environment and the running environment are intentionally connected: a project can live
in the same environment that helped create it, and IDE agents working in Canvas can immediately see
the effects of their changes in the running app — instead of building and running being separate
stages.

## What this is NOT (goal-level)

- NOT a model fine-tuner or trainer. Nulqor shapes *tasks and context*, not weights.
- NOT a benchmark to prove a model is good. It is a workbench to make a model *useful for bounded work*.
- NOT autonomous-agent infrastructure. The human stays in the loop; the agent is a collaborator, not
  a replacement.
- NOT a cloud product. Local-first is a hard constraint, not a default.
- NOT only an agent app, only a coding assistant, only a workflow builder, only a visual canvas, or
  only a knowledge base. It is the workbench underneath all of those.
- NOT a system that requires every project to use every feature.
- NOT a place where speculative future features override the current build phase.
- NOT a system where the core absorbs product behavior.
- NOT finished when v1 ships. It is designed to be built upon, by definition.
