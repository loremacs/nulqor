# Opus 4.8 — Test Audit & Recommendations

Companion to [`OPUS-4.8-REVIEW.md`](OPUS-4.8-REVIEW.md). Covers: are the tests real, is anything
faking green, where coverage is thin, and which tests would keep the project on its stated goal.

---

## 1. Are the tests genuine?

Mostly yes. ~98 test functions across 23 files. A scan for the usual tells — `assert!(true)`,
`assert_eq!(1, 1)`, `#[ignore]`, `unimplemented!`, empty-bodied `todo!()` in tests — found **none**
in test code. The only `todo!()` is in `docs/core-wireframe.rs`, which is explicitly a shape/spec
document, not compiled production code.

Sampled tests are substantive, not decorative:

- `extensions/http-api/src/lib.rs` tests build the real axum router and drive it via
  `oneshot(Request...)`, asserting status codes and parsed JSON bodies
  (e.g. `health_returns_ok`, `transcript_returns_empty_initially`, `register_observer_returns_name`,
  lines 609-651). These exercise real wiring, not mocks of themselves.
- `extensions/validation/src/lib.rs` tests assert pass/fail on concrete inputs (lines 152-168+).
- Core modules (`commands.rs`, `events.rs`, `version.rs`, `permission.rs`, `loader.rs`, `runtime.rs`,
  `capability.rs`) each carry 4-6 focused unit tests.

**No fabricated/green-faking tests found.**

---

## 2. One misleading capability (a "false" check)

This is the most important test-adjacent finding, because it lives in the extension whose entire job
is to be deterministic ground truth.

`extensions/validation/src/lib.rs:105-114` — the `matches_regex` check:

```rust
"matches_regex" => {
    // Simple manual check: does actual match the regex pattern?
    // Uses the `regex` crate if available; falls back to contains for Phase 3.
    let pass = actual.contains(expected);   // <-- substring, NOT regex
    ...
}
```

The comment claims regex (and "uses the `regex` crate if available"), but the code unconditionally
does substring `contains`. So `matches_regex` is byte-for-byte identical to the `contains` check.
A caller validating output against `^\d{4}-\d{2}-\d{2}$` gets a result that has nothing to do with
that pattern. Because `validation:check@1` is what the compounding loop uses to declare a Subject
"pass," a check that misrepresents itself can manufacture a false loop-closure.

This is not a faked *test* — the existing tests honestly test substring behavior — but it is a faked
*feature*: the name and doc promise something the implementation does not deliver.

**Fix (pick one):**
- Implement real matching with the `regex` crate (requires a `Cargo.toml` dependency — per `AGENTS.md`,
  ask before adding), or
- Rename the type to `contains_pattern` and update `PROJECT_FEATURES.md §3.2` so no caller expects regex.

**Related soft check:** `is_date_like` (lines 122-131) passes if any 4-digit year 2000–2099 appears
anywhere in the output. "I was born in 2050" passes; the window also silently breaks in the year 2100.
For the temporal artifact that `3.3` is meant to prove, this validates "a year is present," not "the
date is correct" — adequate as a smoke check, too weak as the proof of loop closure. Strengthen it for
the demo (assert the *expected* current date, not just any year).

---

## 3. Coverage gaps (by risk to the stated goal)

The thesis is "constraint + captured artifacts + deterministic validation make a small model
reliable." The tests should defend exactly that chain. Today the chain is under-tested at its most
load-bearing joints:

1. **No loop-closure regression test.** The project's headline claim (`GOAL.md` loop-closure success)
   has no automated guard. There is nothing that fails if the captured `rules/current-date.md`
   artifact regresses.
2. **No contract-coexistence test across extensions.** `DESIGN.md §4` makes `@1`/`@2` coexistence a
   frozen guarantee; `version.rs` has unit tests, but there is no end-to-end test that registers `@1`
   and `@2` of one command and proves a consumer asking for `@2` is not silently served `@1`.
3. **No lock-across-IO guard.** The sync-RwLock strategy (`OPUS-4.8-PERFORMANCE.md §3`) is safe only
   if no handler holds a write lock across `block_on` network calls. Nothing tests this invariant.
4. **No port/config test.** Nothing asserts the API and llama.cpp defaults differ, so the collision
   in `OPUS-4.8-PERFORMANCE.md §1` was free to exist.
5. **Permission gate at the HTTP boundary.** `permission.rs` is unit-tested, but there is no test that
   a `system`/`destructive` command invoked *through* `/message` or the MCP bridge is actually gated.
6. **Harness token-budget gate** (`DESIGN.md §13`: fixed-context cost reported every turn) — no test
   asserts the budget line item is produced or that it stays under the 8 GB-baseline headroom.

---

## 4. Recommended tests (concrete, ordered)

**Correctness / thesis (do first):**

1. **Loop-closure regression** — assemble the system prompt with `rules/current-date.md` enabled,
   assert the injected date matches today, then run `validation:check@1` with a date-correct check and
   assert pass; flip the rule off and assert fail. This turns the `3.3` proof into a permanent guard.
2. **Contract coexistence** — register `demo:thing@1` and `demo:thing@2`; assert invoking `@2` hits the
   `@2` handler and invoking `@3` fails loud with the available-versions list (per `DESIGN.md §4`).
3. **Validation honesty** — once `matches_regex` is real, test it against a pattern that `contains`
   would get wrong (e.g. `^\d{4}$` vs `"year 2026 ok"`).

**Safety / boundaries:**

4. **Permission gate via HTTP/MCP** — invoke a `destructive` command through the API and assert it is
   refused without confirmation; invoke a disabled-extension command and assert it fails at
   `CommandRegistry::invoke`.
5. **Cross-extension fs boundary** — assert an extension cannot `fs_read` a path outside its declared
   scope (BOUNDARY = ERROR, `DESIGN.md §7`).
6. **Lock-not-held-across-IO** — a provider-state test (or a debug assertion) proving generation does
   not hold the state write lock during the HTTP call.

**Performance (lightweight, not full benchmarks):**

7. **Idle-cost smoke test** — assert chat-panel issues no IPC when hidden/idle (after the polling fix),
   and that click-through does not poll while a stream is active.
8. **Token-budget assertion** — assert the fixed harness context (system prompt + rules + skill index)
   is measured each turn and stays under a configured ceiling, failing loud if it grows past the
   8 GB-baseline headroom in `DESIGN.md §15`.
9. **Provider single-flight** — fire two concurrent generations and assert they serialize (the
   `generation_lock` contract) rather than both hitting the model.

**Process / docs (cheap guards against drift):**

10. Extend `skills/audit-project` to fail when `PHASES.md`'s "current phase" disagrees with `TASKS.md`,
    and when the core-responsibility count differs between `GOAL.md`, `DESIGN.md §2`, and
    `decisions/001`. The product sells consistent process knowledge; let the audit enforce it.

---

## 5. Summary

Tests that exist are honest and meaningful. The risk is not fake green — it is **missing tests at the
exact joints the thesis depends on** (loop closure, contract coexistence, validation honesty,
permission boundaries, token budget) plus one feature (`matches_regex`) that quietly does not do what
its name says. Closing items 1-3 would convert the project's central claims from prose into something
that fails the build when it breaks.
