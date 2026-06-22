# Context Manager

Tracks the token budget for the current transcript and compacts old messages into a
summary when approaching the limit. This is where small-model discipline lives: keep the
context window from silently growing unbounded.

Token counting is approximate (chars ÷ 4 ≈ tokens). It is a budget indicator, not a
billing meter.

## Commands

| Command | Purpose |
|---|---|
| `context:usage@1` | Current token count vs budget (`near_limit`, `pct_used`) |
| `context:set-budget@1` | Update the warning threshold |
| `context:compact@1` | Summarise old messages and hydrate the transcript |

## Events

Subscribes to `transcript:message-added@1` (update running count) and
`transcript:hydrated@1` (recalculate after a hydrate).

Requires: `transcript`.

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest + command declarations |
| `src/lib.rs` | Rust service implementation |
