# Nulqor — Start Here

> **If you are an agent building this project, read this whole file before writing any code.**
> Then read the document for the specific phase you are working on. Do not skip ahead.

> **Self-containment note:** This document set is the only reference you need. A working Go/Wails
> harness previously existed in `harness/` and proved the Phase 2 gateway behaviors. All knowledge
> from that implementation has been extracted into `006-http-api-and-observer-protocol.md`. The Go
> code can be ignored. Do not read it; do not port it; implement the Tauri/Rust architecture from
> scratch using these docs.

> **Location note:** These planning docs currently live in `new-plans/`. When you execute Phase 0.2
> (repo layout scaffold), copy them into the final project at `docs/GOAL.md`, `docs/DESIGN.md`,
> `docs/BUILD_PLAN.md`, `docs/core-wireframe.rs`, and `docs/decisions/001-*.md` through
> `docs/decisions/006-*.md` as shown in the document table below. Until then, read them directly
> from `new-plans/`.

---

## What Nulqor is (in one paragraph)

Nulqor is a **local-first extensible platform** for building apps, tools, automations, and
interfaces — and for systematically improving how AI models perform within them. It runs as a
single desktop app built on a frozen core and an open library of extensions. Extensions are the
unit of everything built on the platform: panels, dashboards, agent loops, domain tools, home
automation surfaces, data pipelines, custom services — each is a bounded, self-contained extension.
Finished configurations can be **compiled into standalone apps** that run outside the platform
(Bundle mode). Work in progress runs live, reachable from within IDE tools like Cursor and
Windsurf over HTTP/MCP (Canvas mode). Inside Canvas, a human and IDE agents collaborate in one
shared conversation against a local model (e.g. Gemma 4 via LM Studio). The point is not to make
any single model smarter. The point is to build a **controllable harness** — small, bounded steps,
the right context, the right tools, and deterministic validation — so that even a limited model can
reliably do useful work, and so that everything learned along the way (skills, rules, scripts,
templates, decisions) is **captured as a reusable artifact** that makes the next task easier. The
platform is designed to get better the more it is used.

## The one sentence that explains the whole product

**A shared evaluation surface where a strong model and a human can watch, probe, and systematically
improve how a weaker model performs bounded tasks — and capture every improvement as a durable
artifact.**

If a design decision does not serve that sentence, it is probably wrong.

---

## The three roles (and who does what)

There are three participants. Keep them straight; confusing them is the most common mistake.

| Role | Who | Job |
|---|---|---|
| **The Builder** | A frontier agent in an IDE (Cursor / Windsurf), plus the human | Builds Nulqor itself and the tools/skills/extensions inside it. This is the **smart** model. |
| **The Subject** | A small local model (Gemma 4 via LM Studio) | The model **under test**. Nulqor exists to make *this* model succeed at bounded tasks. It does NOT build anything. |
| **The Human** | You | Directs the work, judges results, approves changes. Sits in the same shared transcript as the agents. |

**Critical:** The Subject model never has to author an extension to make the system work. The
Builder authors; the Subject is observed. The compounding loop is: Builder + Human watch the Subject
→ find where it struggles → build a tighter step or skill → the Subject does better → capture that as
an artifact. This is the loop. Everything else is mechanism.

---

## How it works (the runtime picture)

```
        ┌──────────────────────────── Nulqor desktop app (Tauri) ───────────────────────────┐
        │                                                                                    │
        │   ┌─────────────┐         ┌──────────────── Rust core ────────────────┐            │
        │   │  Frontend   │◄──IPC──►│ extension loader · event bus · command     │            │
        │   │ (TypeScript)│         │ registry · version mgr · permission gate · │            │
        │   │  panels     │         │ async runtime owner · capability layer     │            │
        │   └─────────────┘         └───────────────────┬────────────────────────┘            │
        │                                               │ everything below is an EXTENSION    │
        │   ┌────────────────────────────────────────── ▼ ─────────────────────────────────┐ │
        │   │ provider-ext · agent-loop-ext · skill-runner-ext · transcript-ext · http-api  │ │
        │   │ persistence-ext · context-mgr-ext · validation-ext · ...                      │ │
        │   └───────────────────────────────────────────────────────────────────────────────┘
        │                                               │                                    │
        └───────────────────────────────────────────────┼────────────────────────────────────┘
                                                         │
                  ┌──────────────────────────────────────┼─────────────────────────────────┐
                  │                                       │                                 │
        Human types in GUI                   IDE agent drives via HTTP/MCP        LM Studio (the Subject model)
        (Builder + Human)                    (the Builder)                       localhost:1234/v1
                  └──────────── all land in ONE shared transcript ───────────────┘
```

Three drivers (human, IDE agent, and any other client) all push turns into **one shared session**.
A turn from the IDE agent appears in the human's window live, and vice versa. The Subject model's
replies appear labeled with who asked. This shared-transcript-with-multiple-drivers is the core
demonstrated behavior — protect it.

> **Build order:** The Rust core (the box labeled "Rust core" above) is built first — Phases 0
> and 1. Nothing in the "EXTENSION" layer below it can exist until the core compiles and passes
> its gate. Every panel, every API, every feature, every app is an extension. The core is the
> only thing that is not.

---

## How humans use it

- Open the app, connect to LM Studio, pick the Subject model.
- Type tasks/prompts to the Subject in the chat box; watch replies with token + latency stats.
- Edit skills, agent personas, and rules in the side panel; save → hot reload → test again.
- Approve or reject agent-proposed changes (draft/review system — nothing persists silently).
- Watch the IDE agent's turns appear in the same window.

## How agents use it

- **The Builder (IDE agent)** connects over the local HTTP API or MCP, registers as an observer,
  sends prompts to the Subject, reads the shared transcript, and — separately, in the IDE — writes
  Nulqor's code, extensions, and skills.
- **The Subject (local model)** just receives turns and replies. It may later be given tools/skills
  by the harness, but it is never required to build anything.

---

## The document set — read in this order

Currently in `new-plans/`; copy to `docs/` and `docs/decisions/` when scaffolding Phase 0.2.

| File (in `new-plans/`) | Final path | Owns | Read when |
|---|---|---|---|
| `README.md` (this file) | `docs/README.md` | Orientation, roles, the one-sentence purpose, the five rules | First. Always. |
| `GOAL.md` | `docs/GOAL.md` | *Why* this exists: the bet, the problem space, principles, roles, modes, success criteria, non-goals, failure modes | Before forming any opinion about scope or *whether* to build something |
| `DESIGN.md` | `docs/DESIGN.md` | *How* it is built: stack, the frozen core, manifest schema, contract versioning, concurrency, the 8 GB budget, small-model rules, quality gates | Before touching the core or any contract |
| `BUILD_PLAN.md` | `docs/BUILD_PLAN.md` | The ordered, step-by-step build with exact tasks and exit gates | Before writing code for a phase |
| `core-wireframe.rs` | `docs/core-wireframe.rs` | The authoritative Rust *shape* of the core (types, traits, signatures) | When implementing any core piece |
| `001-frozen-core.md` | `docs/decisions/001-frozen-core.md` | Why the core list is frozen | When tempted to add to the core |
| `002-contract-versioning.md` | `docs/decisions/002-contract-versioning.md` | Why contracts are versioned and never mutated in place | Before changing any command or event shape |
| `003-events-vs-commands.md` | `docs/decisions/003-events-vs-commands.md` | Events for notification; commands for request-response; bus is namespace-scoped | When designing extension communication |
| `004-concurrency-and-sidecars.md` | `docs/decisions/004-concurrency-and-sidecars.md` | Core owns concurrency; sidecars gated and lifecycle-managed | Before any async work or process spawning |
| `005-stack-choice.md` | `docs/decisions/005-stack-choice.md` | Why Tauri 2 + Rust + TypeScript | If stack rationale is questioned |
| `006-http-api-and-observer-protocol.md` | `docs/decisions/006-http-api-and-observer-protocol.md` | **The complete Phase 2 implementation spec** — HTTP API, WebSocket events, observer/catch-up protocol, MCP tools, message schema, JSONL log, system prompt assembly, skill format, config | **Read this before implementing any Phase 2 task.** Extracted from a working Go harness; supersedes that code. |

> There is exactly one source of truth per concept. The *why* lives in `GOAL.md` and `decisions/`;
> the *how* lives in `DESIGN.md` and `core-wireframe.rs`; the *when/order* lives in `BUILD_PLAN.md`;
> the *proven Phase 2 surface* lives in `decisions/006`. If two documents ever seem to disagree,
> that is a bug — stop and flag it to the human.

## The five rules every building agent must obey

1. **Never put product behavior in the core.** If it can be an extension, it is an extension.
   The core is the frozen list in `DESIGN.md`. Adding to it requires a decision record and human sign-off.
2. **Never mutate a live contract.** To change a command/event/API contract that something depends
   on, publish a *new version* beside the old one. See `docs/decisions/002`.
3. **Stay in your lane.** Work within one extension / one skill / one bounded task at a time.
   Do not edit files outside the task scope. Each extension is a deliberately bounded scope: a
   building agent working on an extension needs context only about that extension and the core
   API — not about every other extension in the system. This bounded scope is what makes
   agent-assisted building on this platform tractable.
4. **Fail loud, never silent.** Surface and log every error, malformed response, and validation
   failure. Empty stubs are honest; fake success is not.
5. **Capture what you learn.** A solved repeatable problem becomes a skill, script, rule, test, or
   decision record — not a thing left in chat history.

When in doubt, do less, ask the human, and write down the question.
