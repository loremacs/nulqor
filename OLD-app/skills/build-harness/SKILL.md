---
name: build-harness
description: Instructions for building the Local LLM Harness application from PLAN.md
triggers:
  - build harness
  - create harness
  - setup harness
---

# Building the Local LLM Harness

This skill documents the process of building the Local LLM Harness application based on PLAN.md.

## Prerequisites

- Go 1.23+
- Wails v2 CLI (`go install github.com/wailsapp/wails/v2/cmd/wails@latest`)
- LM Studio running with Gemma 4 loaded

## Project Structure

```
harness/
  cmd/harness/           # CLI entry point for serve/mcp modes
  internal/engine/       # Core engine (config, loaders, session, prompt)
  internal/lmstudio/     # LM Studio OpenAI-compatible client
  internal/api/          # HTTP/WS API adapter (TODO)
  internal/mcp/          # MCP server adapter (TODO)
  internal/tools/        # Built-in tool implementations (TODO)
  frontend/              # Wails web UI
  harness.toml           # Configuration
  skills/<name>/SKILL.md # User content
  agents/<name>.md       # User content
  rules/*.md             # User content
  runs/                  # JSONL run logs
```

## Build Steps

### 1. Initialize Wails Project

```bash
wails init -n harness -t plain
```

### 2. Create Directory Structure

```bash
mkdir -p internal/engine internal/lmstudio internal/api internal/mcp internal/tools cmd/harness skills agents rules runs
```

### 3. Add Dependencies

```bash
go get github.com/fsnotify/fsnotify github.com/BurntSushi/toml
```

### 4. Implement Core Components

**Config** (`internal/engine/config.go`):
- Load `harness.toml` with sensible defaults
- Support environment variable overrides
- Convert relative paths to absolute

**Session Manager** (`internal/engine/session.go`):
- Manage conversation sessions
- Support forking sessions for A/B testing
- Track messages with metadata (driver, model, latency, tokens)

**Loaders** (`internal/engine/loaders.go`):
- Load skills from `skills/<name>/SKILL.md` with YAML frontmatter
- Load agents from `agents/*.md` or top-level `AGENTS.md`
- Load rules from `rules/*.{md,mdc,txt}`
- Implement `ReadFile`/`WriteFile` with path sandboxing

**File Watcher** (`internal/engine/watcher.go`):
- Use fsnotify to watch skills/agents/rules directories
- Debounce rapid file changes
- Trigger hot-reload on changes

**Prompt Assembler** (`internal/engine/prompt.go`):
- Build system prompt from agent + rules + skill index
- Support injecting specific skill bodies on demand

**Engine** (`internal/engine/engine.go`):
- Tie all components together
- Shared state for GUI, HTTP/WS, and MCP layers

**LM Studio Client** (`internal/lmstudio/client.go`):
- OpenAI-compatible API client
- Support streaming responses
- Model autodetection via `/v1/models`

### 5. Build Frontend UI

**HTML** (`frontend/src/index.html`):
- Top connection bar (endpoint, model dropdown, agent dropdown, badges)
- Center chat transcript
- Bottom input box
- Left panel with skills/agents/rules tree and editor

**CSS** (`frontend/src/main.css`):
- Dark theme matching PLAN.md specs
- Responsive layout with flexbox
- Message bubbles for user/assistant/system

**JavaScript** (`frontend/src/main.js`):
- Connect to backend via Wails bindings
- Handle connection, model selection, messaging
- File editor for skills/agents/rules

### 6. Wire Backend to Frontend

**App** (`app.go`):
- Implement Wails bindings: `TestConnection`, `ListSkills`, `ListAgents`, `ListRules`, `ReadFile`, `WriteFile`, `SendMessage`
- Integrate LM Studio client
- Build system prompts via prompt assembler

### 7. Build and Run

```bash
# GUI mode (default)
wails dev

# Or build binary
wails build

# Server mode (headless)
go run cmd/harness/main.go -mode serve

# MCP mode (stdio)
go run cmd/harness/main.go -mode mcp
```

## Configuration

Edit `harness.toml` to customize:

```toml
[server]
host = "localhost"
port = 8080

[lmstudio]
base_url = "http://localhost:1234/v1"
api_key = ""

[paths]
skills_dir = "./skills"
agents_dir = "./agents"
rules_dir = "./rules"
runs_dir = "./runs"

[defaults]
agent = "default"
model = ""

[generation]
temperature = 0.7
max_tokens = 2048
top_p = 0.9
top_k = 40

[tools]
list_skills = true
load_skill = true
read_file = false
write_file = false
run_shell = false
```

## Next Steps (TODO)

- **M3**: Implement HTTP/WS API server
- **M3**: Implement MCP server with official Go SDK
- **M4**: Add built-in tools (`list_skills`, `load_skill`)
- **M4**: Implement session fork and side-by-side compare
- **M4**: Add JSONL run logging to `runs/` directory

## Troubleshooting

**Build fails with "undefined: NewApp"**:
- Ensure `app.go` is in the root directory, not in a subdirectory
- Wails expects the main package in the project root

**Watcher fails to start**:
- Ensure directories exist before starting
- Check file permissions

**LM Studio connection fails**:
- Verify LM Studio is running on `http://localhost:1234/v1`
- Ensure a model is loaded in LM Studio
- Check that the endpoint URL is correct
