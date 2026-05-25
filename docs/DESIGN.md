# Nulqor — Design

This file owns: **how Nulqor is built** — stack, core responsibilities, manifest schema, contract
versioning, event/command rules, concurrency model, directory layout, quality gates.
It does NOT own: *why* (→ `decisions/`, `GOAL.md`) or *when* (→ `BUILD_PLAN.md`) or *whether* (→ `GOAL.md`).

> This supersedes the earlier DESIGN draft. It keeps everything that was decided and **resolves the
> four seams that were previously under-specified**: (1) contract versioning coexistence,
> (2) named capability instances, (3) event-bus delivery scope, (4) concurrency governance.
> Those four are now first-class. Do not re-open them without a new decision record.

---

## 1. Technology stack

| Concern | Choice | Rationale |
|---|---|---|
| Core binary | **Rust** | Memory safety; the compiler refuses unsafe cross-thread sharing, which is exactly what a 100-extension concurrent system needs. Minimal binary, no runtime on target. |
| App framework | **Tauri 2.x** | Rust backend + web frontend in the OS-native WebView. Small binaries (~5–15 MB). First-class sidecar process management. |
| Frontend | **TypeScript** | Panels are standard web tech; any JS lib usable. |
| Canvas | Single Tauri window (Phase 1) | Lower overhead; multi-window deferred. |
| Async | **Tokio, owned by the core** | One runtime for the whole app. Extensions schedule work *through* the core, never spin their own. |
| Heavy compute | Separate worker pool (tokio blocking pool / rayon), dispatched by the core | CPU-bound batch work must never block the async runtime. |
| Sidecars | Tauri-managed processes, gated behind `system` permission | Python/binaries/scripts spawnable, but lifecycle + timeout + cancellation owned by the core. |
| Local model | LM Studio OpenAI-compatible API at `http://localhost:1234/v1` | Standard schema. **Always fetch the model id from `/v1/models`; never hardcode it.** Treat it as single-flight: one heavy generation at a time → the provider extension owns a request queue. |
| Config formats | TOML (`extension.toml`), `tauri.conf.json`, `Cargo.toml` | TOML supports comments; ecosystem defaults. |
| Distribution | GitHub Releases via Actions, per-OS runners | macOS arm64/amd64, Linux amd64/arm64, Windows amd64. |

---

## 2. Core binary — the frozen list

The core does exactly these things **and nothing else**. This list is frozen. Adding to it requires
a decision record and human sign-off (see `decisions/001`).

1. **Extension loader** — scans `extensions/`, reads each `extension.toml`, runs the linter,
   resolves load order from declared dependencies, loads validated extensions, calls their entry points.
2. **Event bus** — typed, **namespace-scoped** pub/sub. Extensions communicate only through named
   events. No direct extension-to-extension routing. (Scope rules: §6.)
3. **Command registry** — extensions register named, versioned commands with ownership and permission
   declarations. Invocation is by `namespace:action@version` through the IPC bridge. (§5.)
4. **Version manager** — tracks versions of core, schema, API, each extension, **and each individual
   command/event contract**. Validates compatibility on load. Supports old + new contract versions
   coexisting. Reports mismatches before anything renders. (§4 — this is the big one.)
5. **Permission gate** — enforces the four permission classes on every command invocation and every
   capability request. (§5.)
6. **Capability layer** — the only path to the outside world: scoped filesystem access and an HTTP
   client. Extensions request; the core checks scope and performs. Sidecar spawning lives here behind
   `system` permission. (§7.)
7. **Async runtime owner** — owns the one Tokio runtime, grants scheduling to extensions, enforces
   timeouts and cancellation, owns the dispatch boundary to the heavy-compute pool. (§8.)
8. **IPC bridge** — Tauri's Rust/JS layer; routes `invoke` calls to command handlers; exposes the
   (scoped) event bus to frontend extension code.

> Note vs. the earlier draft: "version manager" now explicitly covers per-contract versioning, and
> "permission gate," "capability layer," and "async runtime owner" are called out as distinct core
> jobs rather than left implicit. The spirit (small core) is unchanged; the seams are closed.

---

## 3. Core API (versioned, independent of binary version)

```rust
// All under nulqor_core::api::v1 — extensions declare api-version = "v1" in extension.toml.
register_panel(decl: PanelDecl) -> Result<()>
register_command(decl: CommandDecl) -> Result<()>      // decl carries contract version
publish_event(event: NamespacedEvent) -> Result<()>     // namespace-scoped delivery
subscribe_event(pattern: EventPattern, handler) -> Subscription  // pattern is namespace-scoped
invoke_command(id: CommandId, input: Value) -> Result<Value>     // id includes @version
get_extension_config(key) -> Result<Value>
set_extension_config(key, value) -> Result<()>
get_version_manifest() -> VersionManifest
// capability layer (scoped + permissioned):
fs_read(scoped_path) -> Result<Bytes>
fs_write(scoped_path, bytes) -> Result<()>              // permission >= write
http_request(req) -> Result<Response>                    // host must be declared
spawn_sidecar(spec: SidecarSpec) -> Result<SidecarHandle> // permission == system
// async + compute:
spawn_task(fut)            // runs on the core's runtime, cancellable, with a timeout budget
spawn_compute(job) -> JoinHandle  // runs on the heavy pool, off the async runtime
```

Core maintains backward compatibility for **at least two major API versions**.

---

## 4. Contract versioning — RESOLVED (was seam #1)

There are **three independent version axes**. Do not collapse them.

| Axis | Field | Versions… | Changes when |
|---|---|---|---|
| **API version** | `api-version = "v1"` | The core's function surface (§3) | The core adds/changes API functions |
| **Schema version** | `schema-version = "1.0.0"` | The `extension.toml` manifest *format* | The manifest structure changes |
| **Contract version** | per command/event, e.g. `wiki:get-page@2` | A single command's or event's input/output shape | That one contract's shape changes |

**The rule (frozen): a contract that anything depends on is never mutated in place. You publish a new
version beside the old one.** Concretely:

- A command is registered as `namespace:action@version` (e.g. `wiki:get-page@1`). Two versions can be
  registered simultaneously by the same or different extensions.
- A consumer invokes a *specific* version. If it asks for `@2` and only `@1` exists, the version
  manager fails the call cleanly with a typed error — never a silent fallback.
- An extension may register `@1` and `@2` of the same command during a migration window, then drop
  `@1` once nothing depends on it. The version manager reports who still depends on `@1`.
- Events follow the same rule: `wiki:page-changed@1`, `@2`. Subscribers match a version (or a range).

**Decision rule for *what kind* of change to make** (see `decisions/002`):
- New capability that ALL members of a category can reasonably support, additive, non-breaking →
  widen carefully (rare).
- Same category, needs a shape the old contract lacks → **new contract version** (`@2`). (Common.)
- Genuinely different kind of thing → **new capability/port entirely**, not a version bump.

---

## 5. Capabilities, slots, and named instances — RESOLVED (was seam #2)

Extensions provide capabilities. Capabilities come in two flavors — **do not treat them the same**:

- **Additive** (many active at once): tools, skills, panels, themes, commands. Consumers see all of them.
- **Slotted** (a swappable singleton per *named instance*): providers (model backends), storage/DB,
  memory backend. These register **by name** into a capability slot.

**Named instances (the resolved fix).** A slot is a *named shelf*, not a fixed socket count. A storage
provider registers as, e.g., capability `storage` with instance name `main`. A second registers as
`storage`/`analytics`. Adding a second DB does **not** require changing any port or the core — you
plug in a second provider extension with a different instance name. Consumers ask by name:

```toml
# A storage provider extension declares:
[[provides]]
capability = "storage"          # the slot kind
instance   = "main"             # the named instance on the shelf
contract   = "storage@1"        # which contract version it satisfies
```

```rust
// A consumer asks for a specific named instance satisfying a contract version:
let db = core.resolve_capability("storage", "main", "storage@1")?;
```

If two providers claim the same `(capability, instance)` pair, the linter rejects the install
(conflict = error). If a consumer requests an instance that does not exist, it fails loud.

**Command declaration (now version- and ownership-explicit):**

```toml
[[commands]]
id            = "wiki:get-page@1"      # namespace:action@version — version REQUIRED
owner         = "wiki"
input-schema  = "{ id: string }"
output-schema = "{ content: string, hash: string }"
callable-by   = ["agent", "panel", "service"]
permission    = "read"                 # read | write | destructive | system
```

**Permission classes (unchanged, enforced by the core's permission gate):**
- `read` — safe, no confirmation.
- `write` — modifies state, logged.
- `destructive` — requires explicit user confirmation.
- `system` — core-level / restricted (includes `spawn_sidecar`). Not freely callable.

---

## 6. Event bus delivery scope — RESOLVED (was seam #3)

The bus is **namespace-scoped at delivery**, not broadcast-then-discard. This is what keeps 100
extensions quiet.

- Every event id is `namespace:name@version` (e.g. `canvas:layout-saved@1`).
- A subscription declares the namespace (and optionally name + version range) it wants. The core
  delivers an event **only** to subscribers whose pattern matches. Non-matching extensions are never
  woken.
- Extensions declare the event namespaces they publish and subscribe to in `extension.toml`. The
  linter uses this to (a) validate naming and (b) make the dependency/bake graph statically analyzable.
- **Events are notifications (fire-and-forget, many listeners). Service requests / commands are
  request-response (one named target).** Use events for "something happened that others may care
  about"; use commands for "I need a specific answer from a specific capability now." Choosing wrong
  here is a common design error — see `decisions/003`.

Known Phase 1 events (catalog grows as extensions land):
```
system:ready@1            core finished loading extensions
canvas:ready@1            host workspace mounted
canvas:layout-saved@1     layout persisted
transcript:message-added@1  a turn was added to the shared session
provider:reply-complete@1 the Subject model finished a reply
```

---

## 7. Capability layer — outside-world access

The **only** way an extension touches files, network, or processes:

- `fs_read` / `fs_write` — restricted to paths the extension declared scope for. Cross-extension file
  access is forbidden and linter-enforced (BOUNDARY = ERROR). User/domain file ops belong to a future
  filesystem extension, not to arbitrary extensions.
- `http_request` — only to hosts the extension declared. (LM Studio's localhost endpoint is declared
  by the provider extension.)
- `spawn_sidecar` — `system` permission only. The core owns the spawned process lifecycle: it sets a
  timeout, can cancel/kill, captures stdout/stderr, and surfaces a hung sidecar as a loud failure. A
  spawned process that hangs must NOT be able to wedge the app. (See `decisions/004`.)

---

## 8. Concurrency model — RESOLVED (was seam #4)

The core owns concurrency. Extensions never manage their own threads or runtimes.

- **One runtime.** The core owns a single Tokio runtime. Extensions get async work scheduled via
  `spawn_task`; it is cancellable and carries a timeout budget.
- **I/O is async and cheap.** Model calls, file I/O, network, watchers — all async tasks. Thousands
  can be in flight; the CPU mostly idles because they are *waiting*. An idle CPU during model
  generation is correct: the model is the bottleneck.
- **Heavy compute is dispatched off the runtime.** Batch jobs (eval scoring, embeddings later) go to
  `spawn_compute` on a separate pool, so they never stall the async runtime keeping the UI and chat
  responsive.
- **A slow extension cannot block the core or others.** Every extension task runs isolated with a
  timeout; on overrun the core cancels it and continues. A hung watcher or sidecar degrades to a
  loud, logged failure — never a frozen app.
- **The provider is single-flight.** LM Studio serves ~one heavy generation at a time, so the
  provider extension owns a **request queue**: concurrent callers (human + multiple agents) wait
  their turn cleanly. Do not fire parallel generations at the model and expect parallelism.
- **Shared core state is concurrency-safe by construction** (registry, active capabilities, sessions
  wrapped in the right primitives once, in the core). Rust's compiler enforces no unsafe sharing.

What to build now vs. later: build the owned runtime, safe bus/registry, timeout/cancellation, and
the provider queue **now** (architectural, near-impossible to retrofit). The heavy-compute pool's
*hook* exists now; the CPU-bound extensions that use it come with the ML/eval work later.

---

## 9. Naming conventions

Use **extension** consistently (forbidden synonyms in `.audit-config.json`).

| Term | Meaning |
|---|---|
| Extension | Top-level installable unit with `extension.toml` |
| Panel | UI surface on the canvas |
| Command | Callable, versioned action registered by an extension |
| Capability | A named, possibly-slotted service kind (storage, provider, memory) |
| Skill | Executable process artifact with a contract block (`SKILL.md`) |
| Guide | Instructional Markdown, no contract block — never executed |
| Service | Background extension, no UI panel |
| Provider | Extension exposing a slotted capability (e.g. a model backend) |
| Theme | Visual configuration extension |
| Project | Saved `.nulqor` canvas configuration |
| Bundle | Compiled standalone output baked from a project |
| Canvas | The composable workspace surface |

**Model-facing names stay boring.** Internal naming may be expressive, but anything the Subject model
sees uses plain names: `read_file`, `write_file`, `run_command`, `validate_manifest` — never
`soul:materialize-artifact`. (Small-model rule; see SPEC.)

---

## 10. File & directory rules

```
Max depth: 5 levels from repo root. Level-6 exception ONLY: skills/<name>/scripts/<file>.
Level 7: never. Linter enforces as error.
Allowed extension subdirs: ui/ src/ scripts/ tests/ themes/ generated/ dist/
```

```
nulqor/                          ← 1 (root)
  extensions/                    ← 2
    agent/                       ← 3 (extension root)
      extension.toml             ← 4 (required manifest)
      src/                       ← 4
        commands.rs              ← 5 (normal max)
  skills/                        ← 2/3
    server-inspect/              ← skill folder
      SKILL.md                   ← skill definition
      scripts/                   ← (exception)
        server-inspect.sh        ← 6 (ONLY permitted level-6 location)
```

Uppercase exceptions: `README.md`, `AGENTS.md`, `TASKS.md`, `docs/GOAL.md`,
`docs/DESIGN.md`, `docs/BUILD_PLAN.md`, `docs/decisions/<NNN-title>.md`.

---

## 11. Draft / review system (nothing persists silently)

```
.draft   — new file, never reviewed, not operational. Agents read it as unverified. Never executed.
.review  — modified copy of an existing file; original untouched until approved.
```
Approval is by rename (remove the suffix) by the human. Content-hash in the project file updates on
next save. Soul/wiki changes go only through their extension's `propose-*` commands.

---

## 12. Bake-readiness constraint (protects a future seam — was seam #5)

Baking a subset of extensions into a standalone bundle requires computing the transitive closure of
every command/event/dependency the selected set touches. For that to be *decidable*, the core imposes
one rule **now**, even though bake itself is a later phase:

> **Command and event ids that an extension references must be statically declared in
> `extension.toml`, never constructed dynamically at runtime.** No string-built command ids, no
> computed event names. If references are declarable, closure analysis is a graph walk; if they are
> dynamic, it is undecidable and bake becomes impossible.

The linter enforces this from Phase 1. (See `decisions` index for the bake decision when authored.)

---

## 13. Quality gates (non-negotiable; linter or agent enforces)

- Agent never silently fails a tool call; malformed model responses are caught, logged, surfaced.
- Destructive/flagged actions never run without user confirmation.
- Every skill execution logged: timestamp, name, version, input, output, exit code, outcome.
- Loop iteration limit enforced (configurable 5–50, default 20) — never infinite.
- Linter runs at every install, startup, project load, and bake — never skipped.
- Draft/review files never auto-promoted.
- Extensions never touch another extension's files (BOUNDARY = ERROR).
- Update check is async — never blocks startup.
- Project load shows a compatibility report (API/schema/contract versions) before rendering.
- Provider model id always fetched from `/v1/models` — never hardcoded.
- Harness fixed-context cost (system prompt + rules + skill index) is measured and reported as a
  token budget line item on every turn.

---

## 14. src-tauri layout (fill in against real source as Phase 1 lands)

```
src-tauri/src/
  lib.rs           core entry: loader + bus + registry wiring + runtime ownership
  loader.rs        extension discovery, manifest read, dependency-ordered load
  events.rs        namespace-scoped event bus
  commands.rs      versioned command registry + dispatch
  version.rs       three-axis version manager + per-contract coexistence
  permission.rs    permission gate
  capability.rs    fs/http/sidecar scoped capability layer
  runtime.rs       owned Tokio runtime, spawn_task/spawn_compute, timeouts
  ipc.rs           Tauri invoke routing + frontend bus exposure
src-tauri/tauri.conf.json
src-tauri/Cargo.toml
```

---

## 15. Small-model budget — the 8 GB baseline (a build constraint, not a mode)

The baseline target is a system with ~8 GB VRAM. This is a **whole-system budget**, not just a model
size, and full function at this baseline is the requirement — not a degraded mode.

```
8 GB VRAM budget (approximate):
  Model weights (7B Q4_K_M)      ~4.5 GB
  KV cache / active context      ~1.5–2.5 GB   (varies with context window)
  OS + GPU driver overhead        ~0.5 GB
  Tauri UI rendering              ~minimal
  ─────────────────────────────────────────
  Practical context headroom      ~2,000–6,000 tokens per step
```

Implications that bind the design:
- A single task step loads goal + current files + one skill + one validation command — nothing more.
- Full project docs (GOAL, DESIGN, BUILD_PLAN, all skills, all TODOs) are NEVER all loaded at once.
- Context compaction and handoff files are required infrastructure, not optional features.
- Validation outputs are short and structured — the model reads a result, not raw logs.
- The fixed harness context cost (system prompt + rules + skill index) is measured and reported every
  turn as a budget line item (quality gate, §13). It must not silently crowd out task content.

Larger models and wider context may be used on stronger hardware, but the workbench stays fully
functional at this baseline.

## 16. Harness-minimizes-model-burden rules (apply to skills, tools, agent flows)

These reduce what the model must supply on its own. Derived from current small-model-agent practice.

1. Load only task-relevant context — goal, current files, one skill, one contract, the exact
   validation command. Nothing more.
2. Prefer compact task packs over full-repo context dumps.
3. Split broad goals into bounded steps (e.g. "build the dashboard" → seven sequential manifests +
   stubs + tests, each handled alone).
4. Expose only the 3–5 tools needed for the current step, not 20.
5. Use plain, predictable, model-facing tool names: `read_file`, `write_file`, `run_command`,
   `validate_manifest` — never `soul:materialize-artifact`. (Internal naming may differ; §9.)
6. Include one compact input/expected-output example in each skill.
7. Turn repeated checks into scripts — the model reads `FAIL: extension.toml missing api-version`,
   not 40 lines of raw inspection.
8. Turn repeated failures into validation rules / linter checks / schema constraints.
9. Route hard planning to a stronger model only when needed; classification, template-filling,
   manifest creation, and bounded edits stay on the small model.
10. Treat memory as a retrieval source, not a context dump — select the right small piece, do not
    load wholesale.

The goal: make the model's job as small and clear as possible so a smaller model can succeed at real
work. This is the build-side expression of the GOAL.md thesis (constraint beats capability).
