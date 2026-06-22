# Stack and tooling

Always-on constraints for agents editing this repository. Layout and boundaries live in
`AGENTS.md`; this rule covers stack pins and verification only.

## Stack (check manifests — do not trust training data)

| Layer | Source of truth | Typical pins |
|---|---|---|
| Desktop shell | `src-tauri/Cargo.toml` | Tauri **2**, Rust edition **2021** |
| Frontend tooling | `package.json` | `@tauri-apps/api` **^2**, Vite **^5**, TypeScript **^5** |
| TypeScript compile | `tsconfig.json` | **ES2021**, `strict`, panels under `extensions/**/ui/**/*.ts` |

Read installed versions when it matters: `node -v`, `rustc -V`, `cargo -V`.

## Where code lives

- Extension Rust: `extensions/<id>/src/lib.rs` (never `src-tauri/src/ext_*.rs`)
- Panel UI: `extensions/<id>/ui/` (never root `src/*.ts` for panels)
- Frozen core: `src-tauri/src/` per `docs/DESIGN.md` §14

## Verify after edits

From repo root, run what applies (same commands on Windows, macOS, and Linux):

```powershell
npx tsc --noEmit          # TypeScript panel UI
cargo check --workspace   # Rust
npm start                 # smoke — must work on every OS (via scripts/start-dev.mjs)
skills/audit-skill/scripts/audit.ps1 -SkillName <name> -Quiet   # skill changes
skills/audit-project/scripts/audit.ps1 -Quiet                          # layout / extensions
```

For a full edit loop, load skill `edit-and-verify`.

## Multi-platform

- **Default paths must work on all OSes.** Guard macOS/Windows/Linux-only behavior — see `AGENTS.md` § Multi-platform targeting.
- **Never** put OS-specific shell commands in `package.json` `start`; use `scripts/start-dev.mjs`.
- Host UI: `isMacOS()` / `isWindows()` from `extensions/host/ui/platform.ts`; Rust: `#[cfg(target_os = "...")]`.

## Dependencies

Ask before adding entries to `Cargo.toml` or `package.json`.
