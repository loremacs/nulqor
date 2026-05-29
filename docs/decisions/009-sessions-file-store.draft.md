# 009 — Sessions, file store, human rail, and chat modes (draft)

**Status:** Draft — not accepted; captures design from architecture discussion (2026-05).  
**Supersedes:** nothing yet. Extends `decisions/006` message schema and Phase 4 persistence direction.

**Read this first** when continuing chat/session/group-chat work in a new agent session.

---

## 1. Problem

Users interact with Nulqor through multiple surfaces (chat panel, future CLI panel, HTTP, MCP). They need:

- **Persistent conversations** across app restarts
- **Multiple sessions** (pick up a discussion from days ago)
- **Multi-participant** threads (humans + agents) without losing context rules
- **Human-only navigation** (rail, forks, bookmarks) that must **never poison** model context

Today’s transcript extension is a single in-memory session. Phase 4 planned SQLite; this decision **front-runs persistence with files** and separates agent vs human data on disk.

---

## 2. Core principles

1. **One canonical store, many views** — chat-panel, cli-panel, HTTP, MCP are clients; they do not own history.
2. **Physical separation** — agent contract files vs `human/**` tree; loaders whitelist paths (defense in depth, not filter-in-code only).
3. **Main chat = active branch only** — no fork UI inline; archived paths are human-only.
4. **Thread mode ≠ room mode** — different edit and regeneration semantics (see §5).
5. **Panels are dumb** — session service + commands; UI only renders projections.

---

## 3. On-disk layout (v1 + target)

```
.nulqor/                          # gitignored; under workspace root today
  state.json                      # { "active_session_id": "..." }
  sessions/
    <session-id>.jsonl            # AGENT CONTRACT — active branch messages only
  human/                          # NEVER loaded into generate / MCP context
    catalog.json                  # session list + summaries (v1)
    catalog/INDEX.md              # (planned) human-readable index with links
    rails/
      <session-id>.json           # timeline markers (human navigation)
    branches/
      <session-id>/
        index.json                # fork metadata (v1)
        fork-<uuid>.jsonl         # frozen snapshot at fork time
```

**Session id:** `YYYY-MM-DD-<8-char-uuid>` (v1).

**Agent whitelist:** only `.nulqor/sessions/<active-id>.jsonl` may be read when building model `messages[]`.

**Catalog (target):** each session row: `id`, `title`, `created`, `updated`, `summary`, `mode`, optional `participants`.

**Session file (target):** optional YAML frontmatter block + jsonl body, or sidecar `<id>.meta.json` — v1 is jsonl-only + catalog fields.

**Location (open):** project-scoped `.nulqor/` vs `~/.nulqor/` — not decided; v1 uses workspace root.

---

## 4. What is implemented (v1 — 2026-05)

| Component | Path | Done |
|-----------|------|------|
| Session store extension | `extensions/session-store/` | ✅ |
| File persistence on message + hydrate | `.nulqor/sessions/*.jsonl` | ✅ |
| Catalog | `human/catalog.json` | ✅ |
| Auto rail markers (human/agent turns) | `human/rails/<id>.json` | ✅ |
| User bookmarks | `human-rail:add-marker@1` | ✅ partial (Mark dropdown, Shift+click) |
| Fork on edit (preserve tail) | `human/branches/.../fork-*.jsonl` | ✅ |
| Session picker, load, create | chat-panel + commands | ✅ |
| Main chat + left rail + fork overlay | `extensions/chat-panel/ui/panel.ts` | ✅ |
| Transcript hydrate | `transcript:hydrate@1` (service-only) | ✅ |
| Agent isolation enforcement | capability wall on `human/**` | ❌ convention only |
| Thread vs room modes | | ❌ |
| INDEX.md | | ❌ |
| CLI panel | | ❌ |
| Live streaming in UI | poll 2s | ❌ |

**Enable harness:** `nulqor.toml` — include `session-store`, `transcript`, `chat-panel`, `provider-lmstudio`, etc.; register panel in `extensions/host/ui/panels.ts`.

---

## 5. Session modes

### 5.1 Thread mode (default) — assistant conversation

**Mental model:** one working path with the model; history is **context for the next reply**.

| Action | Behavior |
|--------|----------|
| New message | Append to active branch |
| Edit user message (no replies yet) | Replace in place |
| Edit user message (has replies) | **Fork** — see §6 |
| Edit assistant message | Rare; optional re-run from prior user turn |
| Multi-participant | Many `participant_name` values on one linear timeline |

**Config (target):** `mode: thread`, `branch_policy: preserve | truncate` (default **preserve**).

### 5.2 Room mode — group chat (Discord / Slack style)

**Mental model:** shared channel; history is **the record**, not a single model branch.

| Action | Behavior |
|--------|----------|
| New message | Append only |
| Edit own message | **In-place** + `edited_at` / optional `edit_history[]` |
| Edit | **Never** truncates later messages |
| Edit | **Never** auto-regenerates assistant |
| Fork on edit | **No** |
| Multi-human / multi-agent | Interleaved chronological log |

**Config (target):** `mode: room`, explicit `participants[]` roster (optional v1+).

**UI (not built):** roster, per-speaker labels, in-place edit badges.

---

## 6. Thread mode — fork preservation (designed + partially built)

When user edits message **M** that already has downstream messages:

```
Before:  U1 → A1 → U2 → A2
Edit U2 → U2'

Active after fork:
  U1 → A1 → U2' → (regenerate A2')

Archived file (human/branches/):
  U1 → A1 → U2 → A2   (frozen snapshot)
```

**Steps (atomic):**

1. Snapshot full current messages → `human/branches/<session>/fork-<uuid>.jsonl`
2. Record fork in `human/branches/<session>/index.json` + rail fork marker
3. Replace M, truncate tail on **active** `sessions/<id>.jsonl`
4. `transcript:hydrate@1` + optional regenerate

**Not built:**

- Restore archived branch as active (would fork again)
- Multiple simultaneous live branches
- Branch summaries in agent context (blocked — see §7)

---

## 7. Agent vs human context boundaries

| Scope | Reader | Contents |
|-------|--------|----------|
| **Agent** | `transcript:get`, `provider:generate`, HTTP `/transcript`, MCP | Active branch in `sessions/*.jsonl` only |
| **Human** | chat-panel, future cli-panel | + `human/rails`, + `human/branches`, catalog |

**Rules:**

- No rail JSON, fork registry, or branch files in model context **by default**.
- Archived assistant `reasoning` / wrong conclusions **poison** context — do not inject branch summaries silently.
- **Override (non-default):** per-generate `include_archived_branches: true` or single fork id; must be logged.

**Target enforcement:**

- `human-*` commands: `callable_by: ["panel"]` only — not MCP tools
- Generate path: hard-coded path whitelist
- Optional permission class `human-ui` in core (later)

---

## 8. Human rail (conversation map)

**Purpose:** fast navigation without scrolling; **not** a second transcript for the model.

**Properties:**

- Side panel in chat-panel (v1); same data via commands in CLI (planned)
- **No fork chrome in main chat** — fork rows on rail open overlay / CLI pager
- Each anchor: icon, short preview (~40 chars), `message_id`, optional `fork_id`

**Marker kinds:**

| Kind | Source |
|------|--------|
| `human` / `assistant` / `tool` | Auto on append |
| `fork` | Auto on fork |
| `bookmark` | User (`symbol`: star, flag, question, idea) |
| `verbose` / POI | User or auto on long reasoning/tool dumps (planned) |

**Interactions (target):**

- Click row → scroll/jump to `message_id` in main chat
- Click fork row → `human-branch:open@1` → overlay (UI) or pager (CLI)
- Filters: All | Human | Agents | Bookmarks | Forks (planned)
- Optional **ghost** archived branch column in rail (dimmed) — not built

**Storage:** `human/rails/<session-id>.json` — sidecar, never co-located in agent session file.

---

## 9. Message schema

### 9.1 Visible fields (decisions/006 + extensions)

Per jsonl line: `id`, `role`, `content`, `timestamp`, `participant_name`, `driver`, `model?`, `latency_ms`, `tokens`, `reasoning?`, `agent?`.

### 9.2 Payload block (planned)

Keep `content` human-readable; move machine/audit fields to `payload`:

```json
{
  "payload": {
    "reasoning": "...",
    "tool_calls": [],
    "tool_results": [],
    "system_prompt_hash": "...",
    "raw": {}
  }
}
```

Archived forks store **full payload** for human inspection.

---

## 10. Commands (v1 + planned)

### Agent-safe (existing)

| Command | Owner |
|---------|-------|
| `transcript:get@1` | transcript |
| `transcript:add-user-message@1` | transcript |
| `transcript:hydrate@1` | transcript (service) |
| `transcript:clear@1` | transcript |

### Human / panel (session-store v1)

| Command | Purpose |
|---------|---------|
| `sessions:list@1` | Catalog + active id |
| `sessions:create@1` | New session |
| `sessions:load@1` | Switch active, hydrate transcript |
| `sessions:active@1` | Metadata |
| `sessions:edit-message@1` | Thread edit + fork (user msgs only v1) |
| `human-rail:list@1` | Markers |
| `human-rail:add-marker@1` | Bookmark |
| `human-branch:list@1` | Fork index |
| `human-branch:open@1` | Read archived fork |

### Planned

| Command | Purpose |
|---------|---------|
| `sessions:search@1` | FTS / date filter |
| `human-rail:jump@1` | Explicit jump (CLI) |
| `sessions:set-mode@1` | thread / room |
| Room: `sessions:edit-message-in-place@1` | In-place edit |

HTTP/MCP: mirror agent-safe subset only; human commands stay off MCP tool list.

---

## 11. UI surfaces

### 11.1 Chat panel (`extensions/chat-panel/`)

- **Main:** active branch only
- **Left rail:** Map, You (last human), Mark presets
- **Header:** session dropdown, New, LM Studio connect
- **Overlay:** archived fork (read-only banner: not sent to model)
- **Edit:** user message → fork + optional regenerate confirm

### 11.2 CLI panel (not built)

Same commands; presenters differ:

- `session list`, `session open`, `rail list`, `rail jump`, `branch show [--pager]`

Phased: v0 host commands → v1 chat integration.

### 11.3 Other clients

HTTP `/transcript`, MCP, external IDE agents → **active branch only** (006).

---

## 12. Related systems

| System | Relationship |
|--------|----------------|
| `runs/*.jsonl` | Audit log per day; does **not** restore GUI session. Optional import → sessions (open). |
| Phase 4 SQLite | Indexer over same files; not a second truth store |
| `run-logger` | Subscribes `transcript:message-added`; independent of session-store |
| decisions/006 | Message schema, observer protocol, single session (extended here to multi-session) |

---

## 13. Phasing (suggested)

| Phase | Deliver |
|-------|---------|
| **v1** ✅ | File store, catalog, rail auto + bookmarks partial, fork-on-edit, chat-panel, overlay |
| **v2** | Agent path whitelist, human-only command gates, streaming, symbol picker + filters |
| **v3** | Room mode + roster UI, in-place edits |
| **v4** | INDEX.md, session frontmatter, search/FTS, cli-panel parity |
| **v5** | Restore branch, ghost rail, override flag with logging |

---

## 14. Open decisions

1. **Project vs global** `.nulqor/` location
2. **Auto-title** sessions — first message vs timestamp default
3. **Import** `runs/*.jsonl` into sessions or audit-only forever
4. **Room default roster** — empty vs `[human, active-agent]`
5. **Fork viewer** — overlay (v1) vs popup window for compare-two-paths
6. **One extension vs two** — `session-store` owns all vs split `human-rail` extension

---

## 15. Files to read when implementing

| Path | Role |
|------|------|
| `extensions/session-store/src/lib.rs` | Persistence, commands, fork logic |
| `extensions/transcript/src/lib.rs` | Message schema, hydrate |
| `extensions/chat-panel/ui/panel.ts` | UI |
| `extensions/session-store/README.md` | Quick command reference |
| `docs/PROJECT_FEATURES.md` §2.4–2.4b | Shipped behavior record |
| `BACKLOG.md` § Chat, sessions & group chat | Checkbox backlog |
| `docs/decisions/006-*.md` | HTTP/MCP message schema |

---

## 16. Handoff checklist for next session

- [ ] Read this doc + `PROJECT_FEATURES.md` §2.4–2.4b
- [ ] Run app with `session-store` + `chat-panel` in `nulqor.toml`
- [ ] Test: send → restart → session persists; edit with reply → fork in rail → overlay
- [ ] Pick next backlog item (agent isolation gate vs room mode vs cli-panel)
- [ ] When behavior stabilizes: remove `.draft` and accept decision 009
