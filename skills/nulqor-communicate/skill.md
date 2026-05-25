---
name: nulqor-communicate
description: Talk to a running Nulqor app via HTTP API, MCP, WebSocket, or chat.ps1. Use when the desktop app is running and you need chat or observer access.
---

## Metadata

```text
version:       1.0.0
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
validation:   chat.ps1 health returns ok; errors reported with output
```

---

## Steps

1. Health: `skills/nulqor-communicate/scripts/chat.ps1 -Action health`

2. Register + send:

   ```powershell
   skills/nulqor-communicate/scripts/chat.ps1 -Action register -ObserverName "my-agent"
   skills/nulqor-communicate/scripts/chat.ps1 -Action send -Message "..." -ObserverName "my-agent"
   ```

3. Full surfaces: [REFERENCE.md](REFERENCE.md). Also `scripts/chat.sh` on Unix.

4. After HTTP route changes, update this skill and `docs/decisions/006-http-api-and-observer-protocol.md`.

---

## Verification

- [ ] `chat.ps1 -Action health` succeeds.
- [ ] Observer registered before send when required.
