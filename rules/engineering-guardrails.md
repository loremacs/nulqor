# Engineering guardrails

Always-on correctness and performance constraints for agents editing this repo. Layout/boundaries
live in `AGENTS.md`; stack pins in `stack-and-tooling.md`. This rule covers recurring traps found in
review. Mechanical equivalents run in `skills/audit-project/scripts/audit.ps1 -Strict`.

## Liveness: prefer events over polling

- Push, don't poll. Frontend liveness should come from forwarded bus events (Tauri `emit` →
  `listen`) or the `/ws/transcript` WebSocket — not `setInterval`.
- If you must poll: stop when the document is hidden/blurred, never poll below ~250ms without a
  written reason, and do not discard hash/diff optimizations during streaming.
- The cursor/click-through poll and the chat-panel 2s poll are known debt; do not copy the pattern
  into new panels.

## Validation honesty

- A `validation:check@1` type name must do exactly what it says. `matches_regex` must match a
  regex, not a substring. If behavior cannot match the name, rename the check.
- Validation is the loop's ground truth; a check that misrepresents itself can manufacture false
  loop-closure.

## Concurrency

- Never hold a `std::sync::RwLock`/`Mutex` write guard across a `block_on` network call. Clone or
  copy what you need, drop the guard, then do I/O. A lock held across a generation timeout stalls
  every caller of that provider.
- Generation stays single-flight per provider (`generation_lock`). Do not add parallel generations.

## Ports

- One default listening port per role. The HTTP API and any model-server backend must not share a
  default. Centralize ports in `nulqor.toml` rather than hard-coding new literals.

## Agent loop

- The loop-iteration cap (`docs/DESIGN.md §13`, 5–50, default 20) must ship in the same change as
  the agent loop, with a test proving the cap fires. Never land the loop first and the guard later.

## Docs stay consistent

- When you change phase status or the core-responsibility count, update `docs/PHASES.md`,
  `TASKS.md`, `docs/GOAL.md`, `docs/DESIGN.md §2`, and `docs/decisions/001` together. The audit
  fails on drift between them under `-Strict`.
