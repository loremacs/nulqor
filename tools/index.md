# Tools Index

Developer utilities in the Cargo workspace. Not loaded at runtime as extensions.

| Tool | Path | Purpose |
|---|---|---|
| `nulqor-lint` | `tools/nulqor-lint/` | Validates extension manifests, naming, depth, and boundary rules |
| `mcp-server` | `tools/mcp-server/` | Standalone MCP server binary (dev/testing aid) |

## Commands

```powershell
# Lint one extension
cargo run --manifest-path tools/nulqor-lint/Cargo.toml -- extensions/<id>

# Lint all extensions
cargo run --manifest-path tools/nulqor-lint/Cargo.toml -- extensions/

# Run full workspace tests
cargo test --workspace
```

See `README.md` for prerequisites and `docs/DESIGN.md §13` for quality gates.

## Layout contract

- Dev tools only — not runtime extensions.
- Extension linting: `cargo run --manifest-path tools/nulqor-lint/Cargo.toml -- extensions/`
- Full layout audit: `skills/audit-project/scripts/audit.ps1`
