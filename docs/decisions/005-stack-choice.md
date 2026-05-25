# 005 — Stack: Tauri 2 + Rust core + TypeScript frontend

Status: accepted

## Context
The app needs a thin, extensible core, a flexible UI, a real database, and the option for heavy
compute later. Three desktop stacks were considered: Electron (VS Code / Cursor / Windsurf use it),
Wails (Go), and Tauri (Rust). The reference agent-harness tools in this category (e.g. Hermes and
adjacent projects) independently converged on Rust core + Tauri shell + web frontend + SQLite — that
convergence is signal.

## Decision
Tauri 2.x with a Rust core and a TypeScript frontend. Rationale, mapped to the requirements:
- **Thin extensible core** — Rust traits + a capability/command registry are the cleanest extension
  model of the options. Go's dynamic-plugin story is weak (recompile to extend); this was the one
  place Go was the weaker choice.
- **Powerful DB** — SQLite + FTS5 is first-class (official Tauri SQL plugin).
- **Heavy compute / future ML** — Rust handles CPU-bound work natively; Tauri's first-class **sidecar**
  concept manages a future Python ML service as a managed companion process, not a hack.
- **Small binary** — Tauri ships no browser; ~5–15 MB vs Electron's 150 MB+.
- **Concurrency safety** — Rust's compiler refuses unsafe cross-thread sharing, which directly
  protects a many-extension concurrent system (ADR 004).

## The caveat that did NOT override the decision (record it so it is not re-litigated)
Tauri and Wails both render in the **OS-native WebView** (WebView2 on Windows, WKWebView on macOS,
WebKitGTK on Linux). Consequence: CSS/animation can differ across operating systems, and animation
fidelity is **not** identical cross-OS. The ONLY stack guaranteeing pixel-identical, Chromium-grade
animation everywhere is Electron, because it bundles the browser engine.

This was weighed and rejected as the deciding factor because:
- The primary target is the developer's own machine (one OS), where the cross-OS inconsistency does
  not bite.
- The thin-core / DB / sidecar advantages of Tauri matter more than guaranteed cross-OS motion.

**If the priority ever changes** — i.e. shipping to many users on many OSes with flawless identical
animation as a hard requirement — this decision must be revisited, and Electron becomes the candidate
despite its heavier runtime and weaker extension-core ergonomics. Until then, per-OS UI testing is the
accepted mitigation; keep frontend CSS conservative.

## Consequences
- Building agents write Rust (core/extensions) + TypeScript (panels). Rust's strictness slows the
  build loop slightly but prevents whole classes of concurrency bugs.
- UI must be tested on each target OS before release (quality practice, not a core feature).
