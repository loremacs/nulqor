# Agent Loop

Orchestrates the plan → act → observe → verify → report loop for a bounded task.
Enforces an iteration cap and fails loud when validation fails after all retries are
exhausted (see `docs/DESIGN.md §13`).

## Commands

| Command | Purpose |
|---|---|
| `agent-loop:run@1` | Run a bounded task through the full loop (`task`, optional `skill`, `checks`, `max_iterations`) |
| `agent-loop:status@1` | Query whether a loop is currently running and its iteration |

## Design

Each `run` is synchronous: it blocks the caller until the loop completes or the iteration
cap is hit. Generation is async — the provider returns a `stream_id` and emits
`provider:stream-done@1` on completion; the loop bridges this with a per-invocation
`CondVar` (subscribe before generate, wait, unsubscribe on completion).

Requires: `transcript`, `validation`, `skill-runner`.

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest + command declarations |
| `src/lib.rs` | Rust service implementation |
