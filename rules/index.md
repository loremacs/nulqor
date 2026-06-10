# Rules Index

Runtime context rules loaded by the `context-editor` extension into the system prompt.
Files here are plain markdown (or `.mdc`, `.txt`). `INDEX.md` / `index.md` is skipped at load time.

Read this index before adding or editing rules. Keep entries in sync when rules change.

## Active Rules

| Rule file | Purpose |
|---|---|
| `current-date.md` | Injects `{{current_datetime}}` so the Subject model can answer temporal questions |
| `stack-and-tooling.md` | Stack pins, verify commands, and code-location invariants for self-editing agents |
| `engineering-guardrails.md` | Recurring correctness/perf traps: polling vs events, validation honesty, lock-across-IO, ports, loop cap, doc consistency |

## Adding a Rule

1. Create `rules/<kebab-name>.md` with the rule body.
2. Add an entry to this index.
3. Verify `context-editor` hot-reload picks it up (or restart the app).

## Layout contract

- Rule files live directly under `rules/` (not nested).
- `index.md` is skipped at runtime load — use it only as the registry.
- Every rule file must appear in the Active Rules table above.
