# 004 — The core owns concurrency; sidecars are gated and lifecycle-managed

Status: accepted

## Context
If each extension manages its own threads/runtime and spawns its own processes, a runaway watcher or
hung sidecar can freeze the whole app, and concurrency bugs become unreproducible. The model endpoint
(LM Studio) is single-flight, so naive parallel calls do not help and cause contention.

## Decision
- The core owns ONE Tokio runtime. Extensions schedule async work via `spawn_task` (cancellable, with
  a timeout budget) — they never create their own runtimes/threads.
- Heavy CPU-bound work is dispatched off the async runtime via `spawn_compute` to a separate pool, so
  it cannot stall UI/chat responsiveness.
- A slow/hung extension task is cancelled on timeout and surfaced as a loud, logged failure — never a
  frozen app.
- The provider extension owns a single-flight request queue for the model; concurrent drivers wait
  their turn cleanly.
- `spawn_sidecar` is `system`-permission only. The core owns the spawned process lifecycle: timeout,
  cancel/kill, captured stdout/stderr, loud failure on hang. Every extension does NOT get free process
  spawning — that would route around the permission model and is the largest blast radius.
- Shared core state is concurrency-safe by construction; Rust's compiler enforces no unsafe sharing.

Build the runtime ownership, safe bus/registry, timeouts/cancellation, and provider queue from day
one (architectural, near-impossible to retrofit). The heavy-compute pool's hook exists now; its
CPU-bound consumers come with later ML/eval work.

## Consequences
- One bad extension cannot take down the app.
- Most parallelism budget goes to keeping the app responsive, not racing the (bottleneck) model.
- Sidecar power is available but contained.
