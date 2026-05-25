# Nulqor — Documentation Index

| File | Owns | Read when |
|---|---|---|
| `README.md` (this dir) | Orientation, roles, the one-sentence purpose, five rules | First. Always. |
| `GOAL.md` | *Why* this exists: the bet, problem space, roles, success/failure criteria | Before forming any opinion about scope |
| `DESIGN.md` | *How* it is built: stack, frozen core, manifest schema, contract versioning, concurrency | Before touching the core or any contract |
| `BUILD_PLAN.md` | Ordered, step-by-step build with exact tasks and exit gates | Before writing code for a phase |
| `PHASES.md` | Current phase status and active work | Quick orientation at session start |
| `core-wireframe.rs` | Authoritative Rust shape of the core (types, traits, signatures) | When implementing any core piece |
| `decisions/001-frozen-core.md` | Why the core list is frozen | When tempted to add to the core |
| `decisions/002-contract-versioning.md` | Why contracts are versioned and never mutated in place | Before changing any command or event shape |
| `decisions/003-events-vs-commands.md` | Events for notification; commands for request-response | When designing extension communication |
| `decisions/004-concurrency-and-sidecars.md` | Core owns concurrency; sidecars gated and lifecycle-managed | Before any async work or process spawning |
| `decisions/005-stack-choice.md` | Why Tauri 2 + Rust + TypeScript | If stack rationale is questioned |
| `decisions/006-http-api-and-observer-protocol.md` | Complete Phase 2 implementation spec (HTTP API, WebSocket, observer protocol, MCP) | Before any Phase 2 work |
