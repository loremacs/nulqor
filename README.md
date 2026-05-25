# Nulqor — Self-Contained Product and Architecture Brief

## 1. One-paragraph summary

Nulqor is a local-first, extensible construction platform for building applications, tools, automations, agent workflows, dashboards, and domain-specific workspaces with human and AI collaboration built in from the foundation. It is not a single app, not only an IDE, not only an agent harness, and not only a workflow builder. It is the reusable base layer underneath those things. The core stays small and stable. Everything useful is built as replaceable sealed capability objects: panels, services, model providers, tools, skills, workflows, dashboards, file interfaces, context managers, agent bridges, and bundle/export workflows. A human, IDE agents, local models, scripts, and external tools can all interact with the same running workspace through controlled contracts. Stable arrangements of extensions can be saved as reusable configurations or packaged as focused standalone bundles.

## 2. The short version

Nulqor is a reusable AI-native construction surface.

The IDE/CLI is where code may be written. Nulqor is where reusable building capability accumulates.

The core is the base plate. Extensions are sealed capability objects. Canvas configurations are temporary or saved builds. Bundles are frozen reproducible builds, with standalone export coming later. Humans direct and approve. Agents build, test, refactor, inspect, and propose. Local models can be tested as subjects inside the harness. External IDEs and CLI tools can connect into the same live workspace instead of working in isolated chat windows.

The purpose is to stop starting from zero on every project. Useful process knowledge should become durable artifacts: extensions, skills, scripts, validation checks, templates, rules, run logs, decision records, and saved configurations. Each project should improve the next one.

## 3. Non-negotiables

These rules do not bend. Every architectural decision, phase gate, and extension review should be checked against them.

- The core stays limited to its eight responsibilities.
- Product behavior lives in extensions, not the core.
- Extensions are sealed capability objects, not loose plugin code.
- Builder agents work from strict, versioned scaffolds.
- A repeated fix is incomplete until captured as a named artifact.
- An artifact is not proven until reused on a second related task.
- External IDE and CLI tools may write code. Nulqor accumulates reusable building capability.

## 4. Why Nulqor exists

Modern AI-assisted development is powerful but fragmented. A human often has one conversation in a chat window, an IDE agent in another tool, project files in a repo, scripts in a terminal, memory in scattered notes, and repeated process knowledge trapped in prior sessions. Each new project starts by re-explaining the same context, rules, workflows, and standards.

AI coding tools are good at helping an agent do work inside a project. Nulqor is built to solve a different problem: turning repeated work into reusable building capacity. The question Nulqor answers is not "how do I write this code?" It is "how does what I built and learned this project make the next project faster and more capable?"

Nulqor exists to make the process of building software and tools durable, inspectable, reusable, and composable. The value is not in making one model smarter. The value is in the harness: bounded tasks, precise context, controlled tools, deterministic validation, reusable skills, visible state, and captured artifacts.

The central bet is: constraint beats raw capability for bounded work. If a task is small enough, the right context is supplied, the tools are controlled, and the result is validated, then even a limited model can do useful work. A stronger model benefits too, but the durable asset is the system around the model, not the model itself.

## 5. What Nulqor is not

Nulqor is not a model trainer or fine-tuner. It shapes tasks, context, tools, and validation; it does not change model weights.

Nulqor is not only an IDE, only a coding assistant, only a workflow builder, only an agent harness, only a dashboard tool, or only a knowledge base. It is the workbench underneath those possible tools.

Nulqor is not intended to replace external IDE and CLI coding tools. Those are the primary code-writing surfaces, especially early on. Nulqor is the runtime, coordination surface, and artifact accumulator that makes the work done by those tools compound into reusable capability.

Nulqor is not intended to run every possible feature at once. It should allow different extension sets to be assembled for different jobs.

Nulqor is not an autonomous-agent replacement for the human. The human stays in the loop for direction, judgment, approval, and persistent/destructive changes.

Nulqor is not a cloud-first product. The default assumption is local ownership, local files, local execution, and local models where practical.

Nulqor is not a place where the core absorbs product behavior. Product behavior belongs in extensions.

## 6. The Lego/base-plate model

A useful mental model is a Lego city base plate.

The core is the base plate and connection standard. It defines how blocks attach, how they communicate, how they declare what they need, and how unsafe actions are controlled.

Extensions are the bricks. Each extension is a sealed capability object: a bounded, platform-enforced unit of functionality with a declared manifest, contracts, permissions, commands, events, capabilities, and lifecycle. It is not loose plugin code. Its boundary is enforced by the core, linter, permission gate, version manager, and loader.

Canvas configurations are builds on the base plate. A user can assemble a set of extensions into a useful workspace, save that arrangement, reuse it, fork it, or clear the surface and build something different.

Bundles are frozen or exported builds. A stable canvas configuration can first be frozen as a reproducible bundle profile and may later be packaged as a focused standalone app.

This means Nulqor should not become one giant app. It should become a construction platform where different apps and workflows can be assembled from durable pieces.

## 7. The compounding thesis

The output of a project should not only be the project's code. It should also improve the construction system used for future projects: extensions, skills, rules, templates, validation scripts, context packs, workflow patterns, panel layouts, saved canvas configurations, and eventually bundle recipes.

In Nulqor, a repeated fix is not complete until it is captured as a named durable artifact. A useful skill left only in chat history is unfinished work. A workflow pattern that is not templated is unfinished work. A validation lesson that is not encoded as a check is unfinished work.

The compounding loop only works if improvements are captured.

The first use case is Nulqor helping build Nulqor itself. Each phase of development should leave reusable artifacts — scaffolds, validation suites, skills, workflow templates, saved configurations — that make the next phase faster. If Phase 2 is not measurably easier because of what Phase 1 left behind, the compounding is not working.

## 8. Artifact lifecycle

Saying "fixes must become named artifacts" is only useful if the lifecycle of an artifact is defined. An artifact moves through five stages:

1. **Draft** — proposed by a human or agent. Not yet validated or named.
2. **Validated** — passes a check or demonstrably improves a known task.
3. **Promoted** — named, versioned, and added to the reusable library.
4. **Reused** — helps a second related task. This is the proof it belongs in the library.
5. **Retired or replaced** — superseded when stale, incorrect, or harmful. Old versions are not silently deleted; they are marked retired so the platform knows why they were replaced.

An artifact that has never reached step 4 is a candidate but not yet proven. An artifact that stalls at draft is unfinished work. The platform should make artifact stage visible so humans and agents can tell at a glance what has been validated and what has been proven through reuse.

Promotion must follow the draft/review/approval discipline: agents may propose artifacts, but humans approve promotion into the reusable library. An artifact does not advance from Validated to Promoted without human sign-off.

## 9. The main roles

### Human

The human directs the work, judges results, approves changes, decides when something is useful, and controls destructive or persistent actions. The human can build manually, use agents, or combine both.

### Builder

The Builder is a stronger AI agent, usually running in an external IDE or CLI coding tool. It writes code, creates extensions, edits files, proposes changes, builds tests, reviews failures, and improves the harness. The Builder may connect to the running Nulqor workspace over HTTP or MCP while still editing the repo from an IDE. The Builder is the primary construction agent for Nulqor itself, especially in early phases.

### Subject

The Subject is a local or smaller model under test inside the harness. It is not expected to build Nulqor. It receives bounded tasks, tools, skills, and context and is observed for where it succeeds or fails. When it fails a bounded task, the Builder and Human identify why, then capture a durable improvement: a better skill, clearer rule, validation check, tool wrapper, template, or extension change. The task is rerun. A fix that is not captured as a named artifact is not complete.

### Extensions

Extensions are the primary unit of growth. Each extension is both a technical boundary and a cognitive boundary. A Builder agent working on one extension should only need the core API, the extension contract, and that extension's files — not the whole platform.

## 10. Operating modes

### Canvas mode

Canvas mode is the live construction workspace. Extensions are active, configurable, testable, and composable. Panels, scripts, skills, agents, commands, project state, local models, and external bridges can all operate in one running environment. IDE tools can connect to the running canvas, send messages, inspect transcript state, invoke tools, and see the effects of their changes.

Canvas mode is where humans and agents build, test, and refine systems.

### Bundle mode

Bundle mode is the packaged output. The first version of bundle mode is a frozen configuration profile: exact extensions, versions, contracts, permissions, provider bindings, panel layout, skills, rules, and startup state, all pinned. This is the v1 definition of a bundle. Later, a frozen profile may be compiled into a standalone Tauri app or installer, but that is a later phase and should not be promised in the initial build.

Bundle mode is how a reusable workspace becomes a deliverable app.

## 11. Design principle: small core, large extension surface

The core should be boring, strict, and stable. It is not where product value accumulates. The core owns the minimum mechanics required to safely host and coordinate extensions. Everything else belongs outside the core.

The core does exactly eight jobs:

1. Extension loader
2. Event bus
3. Command registry
4. Version manager
5. Permission gate
6. Capability layer
7. Async runtime owner
8. IPC bridge

No chat system, model provider, database, skill engine, agent loop, workflow editor, dashboard, code editor, transcript store, or app-specific behavior belongs in the core. Those are extensions.

The default answer to "should this go in the core?" is no. Add to the core only when it is truly foundational, cannot be expressed as an extension, and is approved as an architectural change.

## 12. The eight core responsibilities

### 12.1 Extension loader

The loader discovers extensions, reads their manifests, runs the linter, resolves dependency order, and activates valid extensions. Broken extensions fail before they run. The loader is the gate through which all product behavior enters the platform.

The extension lifecycle is:

1. Discover extension folder.
2. Read the manifest.
3. Lint and validate the manifest.
4. Check API, schema, and contract compatibility.
5. Resolve dependencies.
6. Activate the extension with a core context.
7. Deactivate cleanly when needed.

For the first implementation, extensions may be statically compiled in-repo while still using the real manifest, linting, dependency, and activation path. True dynamic loading can come later.

### 12.2 Event bus

The event bus delivers notifications. Events are fire-and-forget messages for "something happened and others may care." Events are namespace-scoped, so only matching subscribers wake up. The system must not broadcast every event to every extension and make them discard irrelevant messages.

Event ids use this shape:

```
namespace:name@version
```

Examples:

```
system:ready@1
canvas:ready@1
transcript:message-added@1
provider:reply-complete@1
```

### 12.3 Command registry

The command registry handles request/response behavior. Commands are for "I need a specific answer or action from a specific capability now." Each command has an owner, permission level, input schema, output schema, caller restrictions, and contract version.

Command ids use this shape:

```
namespace:action@version
```

Examples:

```
hello:ping@1
provider:generate@1
transcript:get@1
workflow:run-node@1
```

A caller invokes an exact version. The system must not silently fall back to the newest version.

### 12.4 Version manager

The version manager protects long-term compatibility. There are three version axes:

- Core API version: the core surface available to extensions.
- Manifest schema version: the shape of extension.toml.
- Contract version: the version of an individual command or event.

A command or event contract must never be mutated in place once anything depends on it. If the shape changes, publish a new version beside the old one. For example, command@1 and command@2 can coexist. Missing versions fail loudly.

### 12.5 Permission gate

The permission gate enforces what extensions are allowed to do.

Permission classes:

- read: safe read-only access.
- write: modifies state and should be logged.
- destructive: requires explicit confirmation.
- system: restricted core-level power, including sidecar spawning.

Every command invocation and capability request passes through the permission gate.

### 12.6 Capability layer

The capability layer is the only route from extensions to the outside world. Extensions do not directly touch arbitrary files, hosts, or processes.

Core-managed capabilities include:

- scoped filesystem read
- scoped filesystem write
- declared-host HTTP requests
- managed sidecar processes

Sidecars allow Python services, binaries, scripts, GPU workers, local engines, and external runtimes to be used without putting their implementation into the core. The core owns lifecycle, timeout, cancellation, stdout/stderr capture, and failure reporting.

### 12.7 Async runtime owner

The core owns concurrency. Extensions schedule async work through the core instead of creating unmanaged runtimes or threads.

The runtime provides:

- spawn_task for cancellable async work with timeout budgets.
- spawn_compute for CPU-bound work on a separate pool.
- sidecar lifecycle control for external processes.
- failure surfacing so one bad extension does not freeze the app.

This supports CPU-heavy extensions later while keeping the app responsive.

### 12.8 IPC bridge

The IPC bridge connects the Rust core to the TypeScript frontend. Frontend panels invoke commands and subscribe to scoped events through the bridge. The frontend should not bypass core permissions or call arbitrary extension internals.

## 13. Extension model

An extension is a sealed capability object: a bounded, platform-enforced unit of functionality with a declared manifest, contracts, permissions, commands, events, capabilities, and lifecycle. It is not loose plugin code. Its boundary is enforced by the core, linter, permission gate, version manager, and loader. The core hosts extensions. It does not understand their product purpose.

Extension kinds include:

- Host: owns the main shell or canvas surface.
- Panel: visual UI panel.
- Service: background service.
- Provider: model, storage, memory, or other swappable backend.
- Tool: callable utility.
- Theme: visual styling package.
- Bake: bundle/export workflow.

An extension implements a small lifecycle:

- manifest(): returns its declaration.
- activate(core_context): registers commands, events, panels, capabilities, and subscriptions.
- deactivate(core_context): cleans up if needed.

## 14. Manifest model

Each extension has a manifest. The manifest is the extension's contract with the platform.

The manifest should declare:

- id
- version
- kind
- core API version required
- manifest schema version
- minimum core version
- required dependencies
- optional dependencies
- capabilities provided
- commands registered
- events published
- events subscribed
- filesystem scopes
- allowed HTTP hosts
- configuration defaults

The linter validates manifests before extensions activate. Invalid shape, missing versions, undeclared references, conflicting capability instances, cross-extension file references, and unsafe patterns fail early.

## 15. Additive and slotted capabilities

Capabilities come in two forms.

Additive capabilities allow many active extensions at once. Examples: panels, tools, skills, themes, commands, validation checks, workflow node types.

Slotted capabilities are swappable named instances. Examples: model providers, storage backends, memory backends, GPU/compute workers. A consumer asks for a specific capability, instance, and contract.

Example provider instances:

- provider/lmstudio satisfying provider@1
- provider/ollama satisfying provider@1
- provider/llamacpp satisfying provider@1
- provider/remote-openai satisfying provider@1

If two extensions claim the same capability and instance, the linter rejects the install. If a consumer asks for a missing instance, the core fails loudly.

## 16. Commands versus events

Commands and events must not be confused.

Use a command when the caller needs a response from a specific target:

- get current transcript
- generate a model reply
- run a workflow node
- read a file through a scoped capability
- validate an extension manifest

Use an event when something happened and subscribers may care:

- canvas mounted
- message added
- provider finished reply
- workflow node completed
- project configuration changed

Commands are request/response. Events are notifications.

## 17. AI and model architecture

Nulqor should support multiple models without making the core a model manager.

Model backends are provider extensions. A local LM Studio provider can be the first provider. Later providers may include Ollama, llama.cpp, remote APIs, specialized local engines, or GPU-backed services.

The provider extension owns model-specific behavior:

- connection details
- model discovery
- request queue
- streaming
- token/latency metadata
- backend-specific errors

The core only resolves the provider capability, routes commands, enforces permissions, schedules work, and delivers events.

Local model endpoints should be treated as constrained resources. If a backend can only handle one heavy generation at a time, the provider extension owns a queue. The core should not fire uncontrolled parallel generations.

## 18. GPU and heavy-compute support

The architecture can support GPU and heavy CPU work without putting GPU logic in the core.

The initial path is sidecars. A GPU service, Python ML worker, C++ binary, Rust compute worker, CUDA service, ROCm service, Metal service, or local inference server can run as a managed sidecar. The core controls lifecycle, permissions, timeouts, and failure reporting.

Later, a dedicated accelerator or compute capability can formalize this:

```
capability = compute
instance = cuda-main
contract = compute@1
```

or:

```
capability = accelerator
instance = local-gpu-0
contract = accelerator@1
```

The core should not become a GPU scheduler. It should provide safe process control, contracts, and capability resolution. Specialized compute belongs in extensions or sidecars.

## 19. UI and app-building model

Nulqor should support many UI tools without becoming one monolithic UI product.

Panels are UI extensions. A workspace may contain a chat panel, node workflow panel, file/project panel, run-log panel, terminal bridge panel, validation panel, model monitor, dashboard panel, or app-specific control surface.

The host extension provides the shell/canvas. Panel extensions register themselves and communicate through commands and events. The IPC bridge routes frontend calls through the core instead of allowing panels to bypass permissions.

This allows different applications to be assembled from different panels and services:

- AI coding harness
- n8n-like workflow builder
- local model evaluation lab
- project dashboard
- home automation control surface
- repo inspection tool
- documentation builder
- standalone focused app

These do not all need to run at the same time. Each canvas configuration selects the blocks needed for the current build.

## 20. External agent and IDE integration

Nulqor should treat external IDEs and CLI tools as first-class participants, not hacks bolted onto chat windows.

An IDE agent can connect to the running Nulqor workspace over HTTP or MCP. It can register as an observer, read the shared transcript, send messages, invoke exposed tools, inspect state, and then edit code in the IDE. Its actions appear in the same live workspace the human sees.

The important idea is shared state. The human, Builder agent, Subject model, and tool outputs should not be scattered across disconnected sessions. They should operate through one controlled surface with visible history and durable artifacts.

## 21. Expected build workflow

Nulqor is not expected to author most of its own code at the beginning. Early development happens in external Builder surfaces: IDE coding agents, CLI coding tools, and similar environments. Those tools write the code. Nulqor provides the architecture that makes the work reusable.

The development loop is:

1. Human identifies a needed capability.
2. Builder agent uses an IDE/CLI coding tool to create or modify an extension.
3. Extension scaffold provides the required structure.
4. Linter validates manifest, contracts, permissions, and boundaries.
5. Tests validate behavior.
6. Nulqor loads the extension.
7. Human and agents use it inside Nulqor.
8. Any useful pattern is captured as a named durable artifact: template, skill, rule, validation check, workflow, or new extension. A fix or pattern that remains only in chat history is considered incomplete.

Strict scaffolds exist because Builder agents need predictable slots. A human can infer missing structure and improvise. An agent is more reliable when it fills a known shape: manifest, commands, events, permissions, UI panel, tests, and validation gates. The scaffold turns extension creation from open-ended software design into constrained completion. This is not a developer convenience. It is how Builder agents become consistently reliable.

The minimum scaffold shape for every extension is:

```
extensions/example-extension/
  extension.toml       ← manifest: id, version, kind, contracts, permissions
  README.md            ← purpose, commands, events, known failure modes
  src-rust/            ← core-side implementation
  src-ui/              ← TypeScript panel (if panel kind)
  tests/               ← behavior tests
  fixtures/            ← sample inputs and expected outputs
```

Every scaffold must include a manifest, command and event declarations, permission declarations, tests, and a validation path. An extension without all five is not considered complete regardless of whether it loads.

Later, Nulqor may include its own local Builder extension: a CLI runner or coding-agent panel inside the workspace. That feature is an extension, not core behavior. Whether code is produced from an external IDE, a CLI tool, or an internal Nulqor agent, all generated changes must flow through the same scaffold, linter, validation, review, and artifact-capture process. If they diverge, the compounding breaks.

## 22. First AI-focused extension set

The first major extension set should prove the AI-native workflow.

Core extensions for that proof:

- provider extension: connects to a local model backend such as LM Studio.
- transcript/session extension: owns the shared conversation state.
- HTTP/WebSocket API extension: lets GUI, IDE agents, scripts, and MCP communicate with the same session.
- chat panel extension: gives the human a visible transcript and input box.
- context editor extension: edits agents, skills, rules, and task context.
- skill runner extension: loads compact skills and executes limited tool loops.
- MCP bridge: exposes selected actions to IDE agents.

This proves the platform's first important behavior: a human, IDE agent, and local model operating in one shared workspace.

The minimum compelling Phase 2 demo is deliberately small:

1. Human sends a message.
2. An external IDE or CLI agent registers as an observer and sends a message.
3. Both messages appear in one shared transcript.
4. The local provider returns a model response visible to all participants.
5. One Subject model failure produces one saved, named skill or rule.
6. The task is rerun. The artifact demonstrably improves the outcome.

If Phase 2 cannot demonstrate step 6, the compounding thesis has not been proven yet. Steps 1–5 are infrastructure. Step 6 is the point.

## 23. Saved configurations and bundles

A canvas configuration is a saved arrangement of extension blocks.

A configuration should capture:

- enabled extensions
- extension versions
- panel layout
- capability bindings
- provider selections
- permission grants
- command/event contracts used
- config values
- attached skills/rules/templates
- external bridges enabled
- validation checks
- bundle/export settings

Bundle v1 is a frozen configuration profile: all of the above pinned to exact versions with a defined startup state. It does not require a standalone installer. It is a reproducible, shareable workspace snapshot. A later phase may compile a frozen profile into a standalone Tauri app or installer; that is out of scope for v1.

For bundling to be possible, command and event references must be statically declared in manifests. If extensions build command names dynamically at runtime, the platform cannot compute the dependency graph for a bundle.

## 24. Draft, review, and approval discipline

Persistent changes should not happen silently.

Agents may propose new files, edits, skills, rules, templates, or extension changes. Risky or persistent modifications should be staged for review. Destructive actions require confirmation. This keeps the human in control while still allowing agents to do useful work.

The platform should prefer:

- draft before commit
- proposal before mutation
- validation before promotion
- explicit failure before silent fallback
- logs for meaningful actions

## 25. Quality and safety rules

Nulqor should follow these rules across the platform:

- Explicit failure over silent failure.
- Destructive actions require confirmation.
- Extensions cannot access another extension's files directly.
- All external access goes through capabilities.
- Sidecars are lifecycle-managed.
- Linter runs before load and before bundle.
- Contracts are versioned and never mutated in place.
- Missing versions fail loudly.
- Tool loops have iteration limits.
- Model responses and tool calls are validated.
- Run logs capture meaningful turns and outcomes.
- Fixed context cost is measured so model budget is visible.
- Stubs fail honestly when a feature is not built.
- A fix or workflow improvement that only lives in chat history is incomplete.

## 26. Build strategy

The build should proceed in narrow phases.

### Phase 0: skeleton and guardrails

Create the Tauri, Rust, and TypeScript skeleton. Open an empty window titled Nulqor. Add the directory layout. Build the linter first. The linter should reject broken extension manifests, bad command/event ids, missing versions, illegal depth, cross-extension references, and dynamically constructed command/event references.

Gate: the app opens, the linter rejects a deliberately broken sample extension, and no product behavior exists.

### Phase 1: frozen core

Implement only the eight core responsibilities: version manager, event bus, command registry, permission gate, capability layer, runtime owner, loader, IPC bridge. Add a host extension and a hello panel extension only to prove the path end to end.

Gate: the sample panel loads through the loader, renders, invokes a versioned command, receives a subscribed event, and a broken extension is rejected before activation.

### Phase 2: first AI harness

Build the provider, transcript, HTTP/WebSocket, chat panel, context editor, skill runner, and MCP bridge as extensions on top of the core.

Gate: human, IDE agent, and local Subject model participate in one shared transcript.

### Phase 3: prove non-chat composability

Build a second useful extension set that is not a chat harness, such as a small workflow-builder, project dashboard, validation console, or file/process automation surface.

Gate: the same core supports a different class of app without product behavior moving into the core.

### Phase 4: saved configurations

Persist canvas configurations: enabled extensions, layout, permissions, providers, skills, rules, bindings, and settings.

Gate: a user can save a build, clear the workspace, restore it, and fork it.

### Phase 5: bundle/export

Package selected configurations as frozen profile bundles. Pin all extension versions, contracts, provider bindings, skills, and startup layout. A later phase may compile this into a standalone app.

Gate: a selected subset of extensions and config restores exactly from a frozen profile without loading the full workbench.

## 27. What can be built on Nulqor

Nulqor is best suited for applications where the hard part is composition, workflow, context, agent coordination, validation, and tooling.

Good fits:

- AI coding harness
- local model testing lab
- skill and rule workbench
- agent task manager
- workflow builder
- project dashboard
- file/process automation tool
- home-lab control surface
- repo inspection tool
- documentation builder
- validation and test dashboard
- app-specific internal tools
- focused standalone bundles

Poorer fits as native Nulqor apps:

- AAA game engine
- full video editor
- professional DAW
- Blender-class 3D editor
- browser engine
- full code editor replacement
- ultra-low-latency graphics app

Even for poorer fits, Nulqor can still orchestrate, inspect, generate assets, run agents, manage workflows, connect to external tools, and package control surfaces. The specialized engine should remain external or be handled through sidecars.

## 28. Language and technology choice

The recommended architecture is:

- Rust core
- Tauri desktop shell
- TypeScript frontend panels
- TOML manifests
- HTTP/WebSocket/MCP external bridge
- Python, C++, Rust, or vendor services as sidecars for specialized compute

Rust/Tauri is the recommended first implementation because the initial core problem is controlled extensibility under concurrency: many extensions, versioned contracts, permission gates, filesystem/network/process access, sidecar lifecycle, async tasks, CPU-heavy jobs, local model queues, and external IDE access.

C++ is useful for specialized performance modules but is riskier as the main agent-coded core. C#/.NET is a strong alternative for Windows-first productivity apps, but less ideal for a small cross-platform systems substrate. Electron/TypeScript is attractive for a VS Code-like UI, but heavier and less safe at the systems boundary. Go is useful for services and CLIs, but weaker for this desktop extensible-core model.

The core should not force every extension to be Rust. Nulqor should be polyglot through sidecars and protocols. Rust governs; TypeScript presents; sidecars specialize.

The long-term durable asset is not the Rust code alone. It is the contracts, tests, scaffolds, validation suites, skills, and workflows. Those artifacts are implementation-independent and could later support an alternate implementation in another stack if the platform's compounding thesis holds.

## 29. The main risks

### Core creep

The biggest risk is putting useful product behavior into the core because it is convenient. This destroys the extension model. The core must remain small.

### Overbuilding before proof

The platform vision is broad. The implementation must stay narrow. Prove the base plate, then one AI-focused build, then one non-chat build, then saved configurations, then bundles.

### Dynamic extension complexity too early

True dynamic loading can wait. The first version can use statically compiled in-repo extensions while preserving the manifest, lint, dependency, activation, command, event, permission, and capability model.

### Context bloat

If every agent sees every file and every rule, the system loses the advantage of bounded work. Extensions and skills must keep context small.

### Silent learning

If fixes stay in chat history, the platform does not compound. Every repeated fix must become a named artifact: script, skill, validation rule, template, decision record, or extension change. Fixes that are not captured are not complete.

### UI scope explosion

Nulqor should support IDE-like panels and workflow surfaces, but it should not attempt a full IDE replacement early. External IDE and CLI tools connect into Nulqor while Nulqor becomes the AI-native orchestration surface.

### Scaffold drift

If extension scaffolds become inconsistent, Builder agents lose reliability. Scaffolds must be kept strict, versioned, and enforced by the linter. An agent that can fill a known shape produces consistent results. An agent working without a scaffold invents structure and produces noise.

## 30. Definition of success

Nulqor succeeds if it becomes easier to build the next useful tool because prior work left reusable blocks behind.

The primary falsifiability test is self-referential: Nulqor is built using Nulqor's own process. If each phase leaves scaffolds, skills, validation suites, and workflow artifacts that make the next phase faster, the compounding thesis is holding. If each phase still starts from scratch, it is not. This is not a future proof of concept — it is the first one.

Defensibility follows from the same logic. A platform where the compounding loop demonstrably works is hard to replicate quickly even when the architecture is understood. The durable asset is not the code. It is the accumulated extensions, scaffolds, skills, validation rules, templates, and workflow patterns that make future work faster. That library takes time to build and cannot be copied without doing the work.

Early success:

- The core loads extensions safely.
- A hello panel proves commands/events/IPC end to end.
- A human and IDE agent can share one transcript with a local model.
- A failed Subject-model task leads to a new skill, validation rule, or tighter step — captured as a named artifact, not left in chat.
- The same artifact helps a second related task.
- Phase 2 is measurably faster because Phase 1 left reusable scaffolds and artifacts.

Platform success:

- Users can assemble different workspaces from reusable extension blocks.
- Agents can build extensions with bounded context using known scaffolds.
- External IDEs and CLI agents can operate against the same live workspace.
- Stable configurations can be saved, forked, restored, and bundled.
- The core remains small while the extension library grows.
- Building the next tool is faster than building the last one.

## 31. Final architectural statement

Nulqor is an AI-native construction platform for composing tools, workflows, app surfaces, and agent harnesses from reusable sealed capability objects. Its core is intentionally minimal: it loads extensions, routes commands, delivers events, manages versions, enforces permissions, controls capabilities, owns concurrency, and bridges the frontend. Everything else is built as an extension.

The IDE/CLI is where code may be written. Nulqor is where reusable building capability accumulates.

The platform exists so humans and AI agents can work in one controlled environment, capture what they learn as durable artifacts, and reuse those artifacts across future projects. It is not a giant app. It is the base plate, contract system, safety layer, and live workbench that lets many focused apps be built, tested, saved, and packaged — and where each build makes the next one easier.