---
name: nulqor-communicate
description: Talk to a running Nulqor app via HTTP API, MCP, WebSocket, or chat.ps1. Use when the desktop app is running and you need chat or observer access.
---

## Metadata

```text
skill_version: 1.0.0
applies_to:    nulqor
topics:        nulqor, http, mcp, chat, observer, api
platform:      all
script_policy: required
scope:         project-scoped
```

Run `scripts/chat.ps1` instead of guessing endpoints. Route tables: [REFERENCE.md](REFERENCE.md).

---

## When to use

- Nulqor desktop app is running (`npm start`).
- Need HTTP/MCP/WebSocket access or a test message.
- LM Studio loaded at `http://localhost:1234/v1` (for model replies).

---

## Contract

```text
when:         Communicating with a running Nulqor instance
inputs:       action, message (send), observer_name, optional model/agent
outputs:      api_json; assistant_reply when send succeeds
side-effects: may send messages and register observers on the running app
validation:   chat.ps1 -Action ready returns ok; send fails fast if provider not connected
```

---

## Steps

1. App + provider: `skills/nulqor-communicate/scripts/chat.ps1 -Action ready`

   Fails fast with a hint if the app is down, LM Studio is unreachable, no model is loaded, or chat-panel never clicked **Connect** (`active` model unset).

2. Connect (if `ready` reports `no_active_model`):

   ```powershell
   skills/nulqor-communicate/scripts/chat.ps1 -Action connect -Url http://localhost:1234/v1
   ```

3. Register + send:

   ```powershell
   skills/nulqor-communicate/scripts/chat.ps1 -Action register -ObserverName "my-agent"
   skills/nulqor-communicate/scripts/chat.ps1 -Action send -Message "..." -ObserverName "my-agent"
   ```

   `send` runs the same provider preflight as `ready` unless `-SkipProviderCheck` is passed.

4. Full surfaces: [REFERENCE.md](REFERENCE.md). Also `scripts/chat.sh` on Unix.

5. After HTTP route changes, update this skill and `docs/decisions/006-http-api-and-observer-protocol.md`.

---

## Verification

- [ ] `chat.ps1 -Action ready` returns `ok: true` with `active` model set.
- [ ] `chat.ps1 -Action send` fails immediately when provider is not connected (no long wait).
- [ ] Observer registered before send when required.
