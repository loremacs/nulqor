# 002 — Contracts are versioned and never mutated in place

Status: accepted

## Context
A contract specific enough to be useful eventually excludes something new. The day a contract must
change, mutating it in place breaks or muddies everything already built against it. This is the
permanent tension of extensibility, and it must be handled by design, not hope.

## Decision
Three independent version axes: API version (core surface), schema version (manifest format), and
**contract version** per individual command/event (`namespace:action@version`).

A contract that anything depends on is NEVER mutated in place. To change it, publish a new version
beside the old one. `@1` and `@2` may coexist; the version manager matches consumers to the version
they request and fails loud if it is absent. An extension may serve both during a migration window,
then retire `@1` once nothing depends on it (the version manager reports dependents).

Decision rule for what kind of change to make:
- Additive, universal, non-breaking → widen carefully (rare).
- Same category, needs a new shape → new contract version `@2` (common).
- Different kind of thing → new capability/port entirely, not a version bump.

## Consequences
- Versioning machinery exists from day one (deliberate; this is a platform, not a throwaway tool).
- No silent fallbacks: a missing version is a typed error, never a guess.
- Migrations are gradual and observable, never big-bang breakage.
