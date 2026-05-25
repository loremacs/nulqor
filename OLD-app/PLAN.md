# Local LLM Harness — Build Plan

A desktop **workbench** for iterating on skills, agent personas, and rules against a
small local model (Gemma 4 in LM Studio). It has a chat UI a human can drive, and it
can also be driven *as the user* by an IDE agent (Cursor / Windsurf) over a direct
connection or MCP — so the IDE can edit the UI/skills/rules, then send test prompts and
observe how the model's behavior changes.

This document is the spec. It contains **no code** — it is meant to be handed to an IDE
coding agent to implement. Decisions, rationale, and known gotchas are included so the
agent doesn't rediscover them the hard way.

---

## 1. Mental model

There are three actors:

1. **The model under test** — Gemma 4, served by LM Studio's OpenAI-compatible API.
2. **The harness** — this app. It holds the *context* (skills + agent persona + rules),
   assembles it into a system prompt, sends conversations to the model, and shows the
   results. It is the thing being tuned.
3. **The driver** — whoever sends user turns. Either a **human** (typing in the chat
   box) or the **IDE agent** (scripting turns to test the model). Both hit the same
   session, so a human can watch the IDE's test conversation live.

The loop you're optimizing: *edit skills/agents/rules → reload → send test prompts →
compare runs → keep what works.* Every feature below exists to make that loop fast.

---

## 2. Tech stack (decided, with rationale)

| Concern | Choice | Why |
|---|---|---|
| Language | **Go** | Single static binary, fast startup, clean cross-compile. |
| Desktop UI | **Wails v2** (stable) | Go backend + web frontend in a native webview, one binary, no Electron. A web frontend is also the thing an IDE agent iterates on fastest. **v3 is alpha** — list it as opt-in only. |
| Frontend | Plain HTML/CSS/JS or a light framework (Svelte or React+Vite) | Keep it simple; the chat UI is not complex. Pick one and stay. |
| Local model API | **LM Studio** OpenAI-compatible server at `http://localhost:1234/v1` | Standard Chat Completions schema; `GET /v1/models` for autodetect; supports tool calling. **Never hardcode a model ID — fetch it from `/v1/models`.** |
| Model | **Gemma 4** (E4B for laptops; 26B-MoE Q4 if you have ~24 GB) | Real model, GGUF, tool-calling capable. Standard system/user/assistant roles. |
| MCP | **Official Go SDK** `github.com/modelcontextprotocol/go-sdk` (v1.x) | Stable; supports stdio and streamable-HTTP transports; typed tools. |

### The IDE gotcha that shapes the design
Cursor and Windsurf lock their **agent** (Composer/Cascade, inline edit, autocomplete)
to their own cloud backend. Their "Override OpenAI Base URL" setting only reroutes the
**plain chat panel**. So do **not** rely on the IDE's model picker to make the IDE talk
to this harness. Instead expose two first-class driver interfaces (Section 6):

- a **direct local HTTP/WebSocket API** (an IDE agent, or the Cline/Continue VS Code
  extensions, can call it), and
- an **MCP server** (Cursor/Windsurf are solid MCP clients, so their agent can call
  harness tools directly).

---

## 3. Architecture

One process. The Wails GUI, the HTTP/WS API, and the MCP server are all thin layers over
a single shared **core engine**. They never talk to LM Studio directly — only through the
engine — so behavior is identical no matter who drives.

```
┌──────────────────────────────────────────────────────────────┐
│  harness  (single Go binary, Wails app)                        │
│                                                                │
│  ┌──────────────┐      ┌───────────────────────────────────┐  │
│  │  Wails GUI   │◄────►│  CORE ENGINE (Go)                 │  │
│  │  (webview)   │ bind │  • config                          │  │
│  │  chat + edit │      │  • loaders + file watcher          │  │
│  └──────────────┘      │  • session manager                 │  │
│                        │  • system-prompt assembly          │  │
│  ┌──────────────┐      │  • LM Studio client (stream)       │  │
│  │ HTTP + WS    │◄────►│  • optional tool/agent loop        │  │
│  │ API (direct) │      └──────────────┬─────────────────────┘  │
│  └──────┬───────┘      ┌──────────────┴─────────────────────┐  │
│         │              │ MCP server (stdio + HTTP transport) │  │
│         │              └──────────────┬─────────────────────┘  │
└─────────┼─────────────────────────────┼────────────────────────┘
          │                             │
   IDE direct / Cline             IDE as MCP client
   (HTTP / WebSocket)             (Cursor / Windsurf)
                       │
                       ▼
        LM Studio  →  localhost:1234/v1  (Gemma 4)
```

**Key rule:** the GUI and every external driver mutate the *same* session through the
engine. A turn injected by the IDE appears in the human's chat window in real time (push
over WebSocket / Wails events), and vice-versa.

---

## 4. Component responsibilities

**Config.** Loads `harness.toml` (LM Studio URL + key, server host/port, paths to
skills/agents/rules dirs, default agent, generation params, which built-in tools are
enabled). Sensible defaults so it runs with an empty file. Environment-variable overrides
for quick tweaks.

**Loaders + file watcher.** Read the three context sources from disk and **hot-reload on
change** (this is core to fast iteration — editing a skill should not require a restart):
- *Skills* — each is a folder with a `SKILL.md` containing YAML frontmatter (`name`,
  `description`, optional `triggers`) + a Markdown body. Optional bundled
  `scripts/ references/ assets/` left on disk for on-demand loading.
- *Agents* — `agents/<name>.md`, frontmatter (`name`, `description`) + persona body.
  Treat a top-level `AGENTS.md` as the default persona too, so it matches the ecosystem
  convention the user already uses.
- *Rules* — `rules/*.md|.mdc|.txt`, concatenated into one always-on instruction block.

**Session manager.** Owns conversations: ordered messages (role + content + metadata like
timestamp, token counts, latency, which agent/model produced it), the active agent, and
the active model. Supports multiple named sessions and a "fork from here" so you can A/B a
prompt change from the same point.

**System-prompt assembly.** Given an agent + rules + skill index, builds the system
message: persona first, then rules, then a compact index of available skills (name +
description) telling the model to load a skill's full body on demand. Keep skill *bodies*
out of the base prompt; inject them only when relevant (selected by the model via a
`load_skill` tool, or auto-injected by trigger match for the chat-only path).

**LM Studio client.** Talks Chat Completions to `/v1`. Streams tokens. Resolves the model
from `/v1/models` when none is pinned. Surfaces clear errors ("server not started",
"no model loaded"). Forwards any tools the caller supplies unchanged.

**Optional tool/agent loop.** When the harness itself is the brain (human-driven chat),
run a short loop so the model can call built-in tools — minimally `list_skills` and
`load_skill` (the heart of the skills feature), plus optional `read_file` / `write_file` /
`run_shell` gated off by default in config. When an external IDE drives and brings its own
tools, the engine stays a pass-through and just enriches the system prompt.

**GUI / HTTP-WS / MCP layers.** Thin adapters. See Sections 5 and 6.

---

## 5. UI layout

A single resizable window, three regions:

1. **Top connection bar (thin).**
   - **Endpoint** field, prefilled `http://localhost:1234/v1`, with a **Connect / Test**
     button and a green/red status dot.
   - **Model dropdown** — see quick-connect below.
   - **Agent dropdown** — populated from the agents dir; switching re-assembles the
     system prompt for the next turn.
   - Small badges: token count of assembled system prompt, last-turn latency.

2. **Center — large chat transcript (dominant area).**
   - Bubbles distinguishing user / assistant / tool-call / tool-result / system.
   - Each turn shows: who drove it (human vs IDE), model name, latency, token usage.
   - Streaming tokens render live. Collapsible "system prompt used for this turn" so you
     can see exactly what the model saw — essential for debugging skill injection.
   - Per-turn actions: **copy**, **regenerate**, **fork session here**.

3. **Bottom — small input box** with Send (Enter to send, Shift+Enter newline) and a
   "driver" indicator showing whether the human or an external client last spoke.

**Left side panel (toggle).** A tree of skills / agents / rules with an **in-app editor**
(plain text/Markdown is enough). Save writes to disk; the file watcher reloads; a toast
confirms "reloaded — N skills." This is the edit-and-iterate surface that lets you (or the
IDE) tweak a `SKILL.md` or `AGENTS.md` and immediately test the effect.

### LM Studio quick-connect (model dropdown behavior)
On Connect/Test, call `GET /v1/models` and **autodetect** whatever is loaded; populate the
dropdown with those IDs (selecting the first by default). Also show a small curated list of
**Gemma 4 defaults** (E2B / E4B / 26B-A4B / 31B) as hints, marked "not loaded" if absent,
so the user knows the intended targets. Never assume a model ID — always use what
`/v1/models` returns. Show a clear, friendly error if the server is down or no model is
loaded.

---

## 6. Driving the harness as "the user" (IDE integration)

Both paths mutate the same session via the engine.

### 6a. Direct HTTP + WebSocket API (simplest; also powers the GUI)
A small local API the GUI uses and any external client (an IDE agent, a script, or the
Cline/Continue extension) can call. Endpoints to provide (described, not prescribed
exactly):
- list / select model; test connection.
- list / switch agent; reload skills+agents+rules.
- list sessions; create / fork session.
- **post a user message** to a session and get the assistant reply (with a streaming
  variant over WebSocket).
- fetch a session transcript.
- read / write a skill, agent, or rules file.

A WebSocket channel pushes every new token and turn to all connected clients (GUI +
external) so views stay in sync.

### 6b. MCP server (so Cursor/Windsurf's own agent can drive it)
Run an MCP server (stdio for local IDE config; offer streamable-HTTP too). Expose tools
that map onto the same engine operations, e.g.:
- `send_message(session?, text)` → returns the model's reply.
- `get_transcript(session?)`.
- `list_skills` / `read_skill(name)` / `write_skill(name, content)`.
- `list_agents` / `set_agent(name)` / `write_agent(name, content)`.
- `write_rules(content)` / `reload`.
- `list_models` / `set_model(id)` / `connection_status`.

Then the user adds this harness as an MCP server in Cursor/Windsurf. The IDE's agent can
now: edit a skill, reload, send a test prompt to Gemma 4, read the transcript, and judge
the result — all without leaving the IDE. This is the cleanest realization of "the IDE
acts as a user."

> Recommend `read_skill`/`write_skill`-style file tools be path-sandboxed to the project
> dirs, and that destructive/shell tools stay opt-in.

---

## 7. The iteration workflow (what success looks like)

1. IDE agent (or you) edits a `SKILL.md` / `AGENTS.md` / a rule via the editor or the
   `write_*` tool.
2. Watcher hot-reloads; the assembled system prompt updates.
3. A test prompt is sent — by you in the box, or by the IDE via `send_message`.
4. The transcript shows the reply **plus the exact system prompt used**, latency, and
   tokens.
5. **Fork session** to re-run the same prompt against a tweaked skill, and eyeball the two
   side by side. (Stretch: a built-in A/B view that runs prompt × two prompt-versions and
   shows both columns.)
6. **Transcript logging**: append every run to `runs/` as JSONL (prompt, system prompt,
   reply, model, params, timestamps). This is what lets you tell "better or worse" over
   time, and lets the IDE agent read past runs to reason about regressions.

---

## 8. Project layout (suggested)

```
harness/
  cmd/harness/           # main: wires engine + GUI + HTTP/WS + MCP; flags for modes
  internal/engine/       # config, loaders, watcher, session mgr, prompt assembly
  internal/lmstudio/     # OpenAI-compatible client (+ streaming)
  internal/api/          # HTTP + WebSocket adapter
  internal/mcp/          # MCP server adapter (official go-sdk)
  internal/tools/        # built-in tool implementations (list/load skill, fs, shell)
  frontend/              # Wails web UI (chat, connection bar, editor panel)
  harness.toml           # config
  skills/<name>/SKILL.md # user content
  agents/<name>.md       # user content (+ optional AGENTS.md)
  rules/*.md             # user content
  runs/                  # JSONL run logs (gitignored)
```

Run modes the binary should support: GUI (default), headless `serve` (HTTP/WS only, no
window — useful for CI or remote), and `mcp` (stdio MCP, for IDE config). All three share
the engine.

---

## 9. Milestones (ordered for the IDE agent)

- **M0 — Scaffold.** Wails v2 project; engine package with config + a stub session;
  window opens with the three UI regions; binary builds clean on your OS.
- **M1 — Connect & chat.** LM Studio client with streaming; connection bar with
  Test/Connect; model autodetect via `/v1/models`; send a message, stream the reply into
  the transcript. (At this point it's a working local chat client for Gemma 4.)
- **M2 — Context + hot reload.** Loaders for skills/agents/rules; file watcher; system-
  prompt assembly with skill index; agent dropdown; left-panel editor that saves and
  reloads; "view system prompt used" on each turn.
- **M3 — IDE as user.** Direct HTTP/WS API (the GUI already uses it); then the MCP server
  with the tool surface in 6b; document how to register it in Cursor/Windsurf and how to
  point Cline/Continue at the HTTP API.
- **M4 — Iteration features.** Built-in `list_skills`/`load_skill` tool loop; session fork
  + side-by-side compare; JSONL run logging in `runs/`.
- **M5 (optional) — Polish.** Multiple sessions/tabs; token budget meter; export a
  transcript; Wails v3 migration if it has left alpha by then.

---

## 10. Risks & gotchas (call these out to the agent)

- **Wails v3 is alpha** — build on v2 unless you accept churn. v3's bindings/IPC differ.
- **Webview quirks** — Wails uses each OS's native webview (WebKit / WebView2 /
  WebKitGTK); test rendering on your actual OS. Keep CSS conservative.
- **LM Studio is single-request-ish** — it serves one heavy request at a time; don't fire
  concurrent generations and expect parallelism. Queue turns.
- **Small-model tool calling is imperfect** — Gemma 4 supports tools, but the smaller
  E2B/E4B will sometimes malform a tool call. The loop must tolerate bad/again JSON: catch
  parse errors, feed the error back as a tool result, cap loop steps. Use Google's
  recommended Gemma 4 sampling params and the current chat template.
- **localhost reachability** — if you ever run the IDE's piece in a sandbox that can't see
  `localhost`, you'll need to bind the harness API on a LAN address (or tunnel). For a
  same-machine setup this is usually a non-issue.
- **Don't pin a model ID** — LM Studio's loaded model changes; always read `/v1/models`.

---

## 11. Decisions to confirm before the agent starts

1. **Wails v2 (stable) vs v3 (alpha)** — plan assumes v2. Change if you want v3.
2. **Frontend flavor** — plain HTML/JS vs Svelte vs React+Vite. (Plain or Svelte = least
   ceremony for a chat UI.)
3. **Default Gemma 4 size for your laptop** — E4B is the safe default; 26B-MoE Q4 if you
   have ~24 GB GPU/unified memory. This only sets the dropdown's highlighted default.
4. **Harness-runs-the-agent-loop vs pure pass-through** — plan does both (loop for GUI,
   pass-through when an IDE brings its own tools). Confirm that's what you want.
5. **Which IDE path first** — MCP (cleanest for Cursor/Windsurf) or the direct HTTP API
   (cleanest for Cline/Continue and scripts). Plan ships HTTP first, MCP second.