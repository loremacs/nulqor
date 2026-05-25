# Project Features

## Nulqor

### Feature name and purpose
Desktop agent harness for testing local models (LM Studio) with custom skills, agent personas, and rules. Supports human chat in a Wails GUI and IDE-driven testing over HTTP/WebSocket.

### Files involved
- `harness/main.go` — Wails GUI entrypoint
- `harness/app.go` — Wails bindings (connect, chat, file editor)
- `harness/cmd/harness/main.go` — CLI entry for `serve` and `mcp` modes
- `harness/harness.toml` — Configuration (paths, LM Studio, generation, tools)
- `harness/internal/engine/` — Core engine (config, loaders, sessions, prompt, chat, tools)
- `harness/internal/lmstudio/client.go` — OpenAI-compatible LM Studio client
- `harness/internal/api/server.go` — HTTP/WebSocket API
- `harness/frontend/src/` — Chat UI (HTML/CSS/JS)
- `skills/`, `agents/`, `rules/`, `AGENTS.md` — User-editable context at repo root

### Key functions/components
- `engine.LoadConfig(path)` — Loads `harness.toml`; resolves paths relative to config file directory
- `engine.Loaders` — Loads skills/agents/rules; YAML frontmatter via `parseFrontmatter`
- `engine.PromptAssembler.AssembleSystemPrompt(agent)` — Persona + rules + skill index
- `engine.Engine.SendChat(ctx, client, ChatRequest)` — Session-aware chat with tool loop and JSONL logging
- `engine.ToolRegistry` — Built-in `list_skills` and `load_skill` tools
- `lmstudio.Client.ListModels / ChatCompletion / ChatCompletionStream`
- `api.Server` — REST + WebSocket endpoints over shared engine state
- Wails bindings: `TestConnection`, `GetSettings`, `SendMessage`, `SendMessageStream`, `GetSystemPrompt`, `ReadFile`, `WriteFile`

### Database interactions
None. Run logs append to `harness/runs/YYYY-MM-DD.jsonl`.

### Validation and business logic
- Skill/agent frontmatter must be YAML between `---` markers
- `INDEX.md` files in agents/rules dirs are ignored
- `AGENTS.md` at workspace root (parent of `agents/`) is always loaded as the default agent
- File read/write sandboxed to skills/agents/rules directories
- Tool loop capped at 8 steps; malformed tool calls return error text to the model
- Model ID autodetected from LM Studio when not pinned

### UI interactions
- Top bar shows connection status, active model/agent chips, token/latency badges, and a Settings button
- Settings panel (in-window slide-over): LM Studio endpoint + connect, model/agent selects, generation params (read-only from `harness.toml`), live API URL, reload context, paths
- Send uses `SendMessageStream` with Wails `chat-stream` events; prompts to open Settings if not connected
- Each assistant turn shows collapsible system prompt used
- Assistant turns with model reasoning show a collapsible **Thinking** block (`reasoning_content` from LM Studio), streamed via `chat-reasoning-stream` / transcript `reasoning_delta`, collapsed when complete
- Left panel lists skills/agents/rules; editor saves via `WriteFile` and reloads context

### Dependencies
- Go 1.23+, Wails v2, LM Studio OpenAI-compatible API
- `gopkg.in/yaml.v3`, `github.com/BurntSushi/toml`, `github.com/fsnotify/fsnotify`, `github.com/gorilla/websocket`

### Change log
- 2026-05-24: Fixed YAML frontmatter parsing (was incorrectly using TOML). Fixed default agent loading blocked by `INDEX.md`. Wired sessions, tool loop, streaming, HTTP API, and JSONL run logging.
- 2026-05-24: First observer register starts at ack seq 0 (full backlog on first catch_up). Catch-up log dedupes to message_added only (no stream_start/done duplicates).
- 2026-05-24: Renamed app display name to Nulqor (window title, UI, agent persona).
- 2026-05-24: Model reasoning/thinking UI — captures `reasoning_content` from LM Studio (stream + non-stream), stores on message metadata, emits `reasoning_delta` / `chat-reasoning-stream`, collapsible Thinking block in GUI.
