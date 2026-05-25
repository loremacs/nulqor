# 001 — The core is frozen at eight responsibilities

Status: accepted

## Context
A platform that grows by extensions only works if the foundation does not move. If "useful" things
keep entering the core, the core stops being frozen, and every change risks the whole system.

## Decision
The core does exactly the eight things in `DESIGN.md §2` and nothing else: extension loader, event
bus, command registry, version manager, permission gate, capability layer, async runtime owner, IPC
bridge. The core contains NO product behavior — no model, no chat, no DB, no skill engine, no agent
loop. Those are all extensions.

Adding anything to the core requires a new decision record AND explicit human sign-off. The default
answer to "should this go in the core?" is no.

## Consequences
- The core stays small enough to specify completely and freeze.
- Product value accumulates in extensions and captured artifacts, not in the core.
- Some things are slightly less convenient (e.g. logging is an extension, not a core freebie). This is
  the intended trade: convenience now is fragility later.
