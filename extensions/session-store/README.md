# session-store

File-backed chat sessions and human-only navigation data.

## Layout

```
.nulqor/
  state.json                 # active session id
  sessions/<id>.jsonl        # agent contract — active branch messages only
  human/
    catalog.json             # session list + summaries
    rails/<id>.json          # timeline markers (human UI / CLI)
    branches/<id>/           # archived fork snapshots (never agent context)
      fork-<uuid>.jsonl
```

## Commands

| Command | Callable by | Purpose |
|---------|-------------|---------|
| `sessions:list@1` | panel | List sessions from catalog |
| `sessions:create@1` | panel | New empty session, set active |
| `sessions:load@1` | panel | Load session into transcript |
| `sessions:update@1` | panel | Edit catalog title and summary |
| `sessions:delete@1` | panel | Delete session files; switch active if needed |
| `sessions:active@1` | panel | Active session metadata |
| `sessions:edit-message@1` | panel | Edit user turn; fork-preserve when tail exists |
| `human-rail:list@1` | panel | Markers for active session |
| `human-rail:add-marker@1` | panel | User bookmark on a message |
| `human-branch:list@1` | panel | Fork metadata for session |
| `human-branch:open@1` | panel | Read archived fork transcript |

Agent-facing paths read **only** `.nulqor/sessions/*.jsonl` via transcript. `human/**` is never mounted into generate/MCP context.

**Design spec (draft):** [`docs/decisions/009-sessions-file-store.draft.md`](../../docs/decisions/009-sessions-file-store.draft.md) — full context for thread vs room mode, rail, forks, and unimplemented ideas.
