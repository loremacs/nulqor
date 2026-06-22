# Decision Records

Captures architectural decisions as `docs/decisions/<NNN>-<slug>.md`. Auto-numbers from
existing files so the builder never has to count by hand.

## Commands

| Command | Purpose |
|---|---|
| `decisions:create@1` | Write a new ADR (`title`, `context`, `decision`); returns path + number |
| `decisions:list@1` | List existing decisions (number, title, status, path) |

## Notes

- Filesystem scope: `docs/decisions/` (declared via `fs-scopes` in the manifest).

| Path | Purpose |
|---|---|
| `extension.toml` | Manifest + command declarations |
| `src/lib.rs` | Rust service implementation |
