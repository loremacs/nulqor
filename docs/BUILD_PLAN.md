# Nulqor — Build Plan

This file owns: **the ordered steps to build Nulqor**, with exact tasks, exit gates, and guardrails.
It does NOT own: *why* (→ `GOAL.md`, `decisions/`) or *how the pieces are shaped* (→ `DESIGN.md`).

> **For the building agent:** Work top to bottom. Do not start a phase until the previous phase's
> **exit gate** is green. Do not work on more than one task at a time. Each task says exactly what to
> build and what "done" means. If a task is ambiguous, STOP and ask the human — do not guess.

---

## Context for building agents

A working Go/Wails harness proved the Phase 2 gateway behavior on 2026-05-24. That code lives in
`harness/` at the repo root and **can be ignored** — `decisions/006` extracts the entire proven
surface (HTTP API, WebSocket events, observer protocol, MCP tools, message schema, JSONL log
format, system prompt assembly, skill format, tool loop, config shape, and gotchas). Read it
before touching Phase 2. Do not deviate from the surface it documents without a new ADR.

The run log at `harness/runs/2026-05-24.jsonl` contains the live evidence for the Phase 2 gate.
The unit tests at `harness/internal/...` are the automated evidence. Both are now superseded by
this build plan and `decisions/006`.

---

## How to use this plan (read first)

**The build order is non-negotiable: Rust core first, everything else second.**

Phases 0 and 1 build the Rust core. That core — the eight responsibilities in `DESIGN.md §2` — is
the literal prerequisite for everything else. No extension can be loaded, no panel can render, no
HTTP API can exist, no app can be built until Phase 1's gate is green. All product behavior, all
features, all apps, all AI capabilities, all UI panels: every single one is an extension built on
top of the core in Phase 2 and beyond. The core does not contain any of them. This is not a
preference; it is a hard architectural constraint. If you find yourself writing product logic before
Phase 1's gate is green, stop.

- **One task at a time.** Finish it, prove it, then take the next. Never batch.
- **Stay in the lane** the task defines. Touching files outside scope is a defect, even if "helpful."
- **Every phase ends with a gate.** A gate is a checklist. If any item is red, the phase is not done.
- **Build stubs honestly.** If a feature is not in this phase, leave a stub that fails loud, not a
  fake that pretends to work.
- **When you finish a task, write a one-line entry** in `docs/PROJECT_FEATURES.md` (what shipped) and, if
  you made a non-obvious choice, a decision record in `docs/decisions/`.

Legend: ◻ = task, ✅ = exit-gate item.

---

## Phase 0 — Skeleton & guardrails (no behavior yet)

Goal: a Tauri+Rust app that opens an empty window and a linter that already enforces the rules, so
every later phase is born compliant.

◻ **0.1** Create the Tauri 2.x + Rust + TypeScript project. Window opens, titled "Nulqor". Nothing else.
◻ **0.2** Set up the repo layout from `DESIGN.md §10`. Add `README.md`, `docs/` set, empty `extensions/`,
   empty `skills/`.
◻ **0.3** Write the **linter** as the first real code. It enforces, mechanically:
   - `extension.toml` presence + schema (`DESIGN.md §5`)
   - id / command / event naming incl. `@version` (`§4`, `§6`)
   - directory depth rules (`§10`)
   - boundary rule: no cross-extension file refs (`§7`)
   - static-declaration rule for command/event refs (`§12`)
   It runs on a folder and prints **structured, exact** failures (`FAIL: <file>: <reason>`), never prose.
◻ **0.4** CI: GitHub Actions builds the app on all target OSes and runs the linter. Red CI blocks merge.

✅ **Gate 0:** App opens on your machine. Linter runs and correctly rejects a deliberately-broken
   sample extension. CI is green. No product behavior exists yet — that is correct.

---

## Phase 1 — The frozen core (the eight responsibilities)

Goal: implement the core from `DESIGN.md §2`, and *only* the core. No providers, no chat, no UI panels
beyond a host. This phase is where the four resolved seams get built in — get them right now.

> Build order matters: version manager and event bus underpin everything, so they come first.

◻ **1.1 Version manager** (`version.rs`) — three axes (api / schema / contract), per-contract
   coexistence, compatibility report. Unit tests: `@1` and `@2` of a command coexist; requesting a
   missing version fails loud. (`DESIGN.md §4`)
◻ **1.2 Event bus** (`events.rs`) — namespace-scoped delivery. Non-matching subscribers are NOT woken.
   Test with 50 dummy subscribers across namespaces; assert only matching ones receive. (`§6`)
◻ **1.3 Command registry** (`commands.rs`) — register/invoke by `namespace:action@version`, with
   ownership + permission. (`§5`)
◻ **1.4 Permission gate** (`permission.rs`) — the four classes; `destructive` requires confirmation
   hook; `system` restricted. (`§5`)
◻ **1.5 Capability layer** (`capability.rs`) — scoped `fs_read`/`fs_write`, declared-host `http_request`,
   `spawn_sidecar` behind `system` with owned lifecycle + timeout + kill. (`§7`)
◻ **1.6 Async runtime owner** (`runtime.rs`) — one Tokio runtime; `spawn_task` (cancellable, timed);
   `spawn_compute` hook to a separate pool. (`§8`)
◻ **1.7 Extension loader** (`loader.rs`) — discover, read manifest, run linter, dependency-order load,
   call entry points. Lazy activation hook (load on first need). (`§2.1`)
◻ **1.8 IPC bridge** (`ipc.rs`) — Tauri invoke routing to commands; expose scoped event bus to frontend.
◻ **1.9 Host extension** — the one default extension that mounts the canvas/window shell. Emits
   `canvas:ready@1`. This proves the loader + bus + IPC end-to-end.
◻ **1.10 Sample panel extension** — trivial "hello" panel that registers a panel, subscribes to one
   event, registers one `read` command. Proves the extension contract works from both Rust and TS sides.

✅ **Gate 1:** Core compiles. All §2 responsibilities exist and are unit-tested. The sample panel
   loads via the loader, renders, its command is invokable, and it receives its subscribed event.
   A broken sample is rejected by the loader's linter pass before load. The core contains NO product
   behavior (no model, no chat, no DB) — verify against the frozen list.

---

## Phase 2 — The Subject connection (provider + shared transcript)

Goal: reproduce and harden the demonstrated behavior — human + IDE agent + Subject model in one shared
transcript — but now built *as extensions on the Phase 1 core*, not baked in.

> **Reference:** `decisions/006` is the complete spec for this phase. Every task below names the
> relevant section. Implement to that surface; do not invent a different API shape.

◻ **2.1 Provider extension** — slotted capability `provider`, instance `lmstudio`, satisfying
   `provider@1`. Connects to LM Studio; fetches model id from `GET /v1/models` (never hardcoded);
   streams replies via the Chat Completions API; owns the **single-flight request queue** so
   concurrent callers wait their turn cleanly. (`DESIGN.md §5`, `§8`, `decisions/004`,
   `decisions/006 §1` endpoints `/connect`, `/models`)

◻ **2.2 Transcript / session extension** — the one shared in-memory session. Messages carry the
   schema in `decisions/006 §5`: `id`, `role`, `content`, `timestamp`, `model`, `latency_ms`,
   `tokens`, `driver`, `participant_name`, optional `reasoning`. Participant naming rules per
   `decisions/006 §4`. Emits `transcript:message-added@1` and the other event types in
   `decisions/006 §2`. Maintains `transcript_hash`. Appends one JSONL line per assistant turn
   per `decisions/006 §8`.

◻ **2.3 HTTP + WebSocket API extension** — implements the **exact** endpoint surface in
   `decisions/006 §1`, WebSocket event types in `§2`, and observer/catch-up protocol in `§3`.
   Critical behaviors: first `catch_up` after register returns full backlog; catch-up log contains
   `message_added` only (no stream fragments); duplicate observer name is idempotent; unregistered
   `observer_name` on `/message` → 400. This is how the IDE agent drives the Subject.

◻ **2.4 Chat UI panel** — dominant transcript view + input box + connection bar (endpoint field,
   model dropdown autodetected from `/v1/models`, agent dropdown). Streams reply tokens live.
   Shows per-turn collapsible system prompt. Shows model reasoning in a collapsible "Thinking"
   block when present. Displays **fixed harness token cost** (system prompt size) as a budget
   line item per turn. Participant display per `decisions/006 §4`. System prompt assembly per
   `decisions/006 §6`.

◻ **2.5 Context editor panel + skill runner** — skills/agents/rules tree + in-app editor + save
   → hot reload (file watcher as an async task on the core runtime). Skill YAML frontmatter format
   and tool loop (cap, error handling) per `decisions/006 §7`. Config shape per `§10`.

◻ **2.6 MCP bridge** — stdio MCP proxy to the HTTP API. Exact tool surface per
   `decisions/006 §9`: `register_observer`, `catch_up`, `ack_observer`, `send_message`,
   `list_observers`. Does not embed the engine; app must be running.

✅ **Gate 2:** Human (GUI) and an IDE agent (HTTP or MCP) hold ONE shared conversation with the
   Subject model in one window, each turn labeled with its driver, with token+latency stats. Editing
   a skill hot-reloads. The observer/catch-up protocol works exactly as `decisions/006 §3` specifies:
   new observer sees full backlog on first catch-up; subsequent catch-ups return only new turns;
   `message_added`-only log with no duplicates. Everything in this phase is an extension; the core
   was not modified. **This gate was already demonstrated by the Go harness on 2026-05-24 and is
   now the acceptance baseline for the Tauri rebuild.**

---

## Phase 3 — Prove the loop closes (the thesis test)

Goal: demonstrate the compounding loop on the smallest possible real case **before** building more.
This is the most important phase. If it fails, stop and rethink before investing further.

◻ **3.1 Skill runner extension** — loads `SKILL.md` skills, injects the matching skill into context on
   demand (the `load_skill` tool path), logs every execution.
◻ **3.2 Validation extension** — runs a deterministic check on a task result (e.g. "is the output valid
   JSON matching this schema?") and returns a short structured pass/fail the model can read.
◻ **3.3 The closure demonstration (do this by hand, with the human):**
   1. Pick a bounded task the Subject model currently **fails**.
      **Known proven candidate (from `harness/runs/2026-05-24.jsonl`):** The Subject (Gemma 4 E4B)
      consistently fails temporal questions — "what day is it", "what year is it" — by replying
      it lacks real-time information. This failure appeared in turns 3, 6, 7, 8, and 18 of the
      run log. It is bounded, deterministic, and easy to verify.
   2. The Builder authors ONE artifact. For the temporal candidate, the artifact is a **rule or
      skill** that injects the current date into the system prompt context.
   3. Show the Subject now **passes** the task (correctly states the date).
   4. Show the artifact persists and helps a second related temporal task (e.g. "what time of
      year is it?") without rework.
   5. Confirm the whole thing stayed within the 8 GB token budget.
◻ **3.4 Run logging** — append every turn to `runs/YYYY-MM-DD.jsonl` (prompt, system prompt, reply,
   model, params, timings) so better-or-worse is *visible*, not felt.

✅ **Gate 3:** A documented before/after where one captured artifact turned a Subject failure into a
   repeatable success, within budget, with the run logs to prove it. The compounding loop is real.

---

## Phase 4 — Persistence & the harness essentials

Goal: stop losing state, and stand up the minimum harness from the SPEC (task scope + commands +
relevant skill + guardrails + verification + logging).

◻ **4.1 Persistence extension** — SQLite (+ FTS5) storing sessions, runs, and artifacts so closing the
   app no longer wipes work. Slotted `storage` capability, instance `main` (proves named instances).
◻ **4.2 Project save/load** — `.nulqor` project files; compatibility report (version axes) on load.
◻ **4.3 Agent-loop extension** — plan → act → observe → verify → report, with the enforced iteration
   cap and loud failure handling. Uses skill runner + validation + provider.
◻ **4.4 Context manager extension** — loads only task-relevant context; reports/compacts when nearing
   the token budget. This is where the small-model discipline lives.
◻ **4.5 Decision records workflow** — a lightweight command to capture a decision as
   `docs/decisions/<NNN>.md`. (The loop's capture step, made easy.)

✅ **Gate 4:** Restart the app → prior session restores. A bounded task runs through the agent loop with
   verification and full logging. A second `storage` instance can be added without touching core or the
   first one (named-instance proof).

---

## Phase 5+ — Grows as extensions only (no core changes)

Everything beyond here is additive extensions, each its own bounded task, each with its own gate:
A/B compare view, bake/export workflow, memory/wiki, linter UI, model routing, domain builders
(dashboards, home automation, etc.), heavy-compute eval scoring (the reserved compute pool finally
gets used), optional ML sidecar.

> If any Phase 5+ item seems to need a core change, that is a red flag: stop, write a decision record
> proposing the change, and get human sign-off. The default answer is "make it an extension."

---

## Cross-phase guardrails (apply to every task, always)

1. **Core is frozen.** Adding to the §2 list needs a decision record + human sign-off. Period.
2. **Never mutate a live contract.** New version beside the old (`@2`), never edit `@1` in place.
3. **One lane at a time.** One extension / one skill / one task. No drive-by edits.
4. **Fail loud.** Surface + log every error and malformed response. Honest stubs over fake success.
5. **Capture learning.** Repeatable fix → skill / script / rule / test / decision record.
6. **Boring model-facing names.** `read_file`, not `qor:phase-realize`.
7. **Budget is real.** Watch the fixed harness token cost; it is a line item, not free.
8. **Confirm destructive actions.** Never persist or destroy without the human's explicit ok.
9. **Ask when unsure.** A written question beats a confident wrong guess. The building agents are not
   expected to resolve ambiguity alone — that is what the human is for.
