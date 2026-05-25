import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "./style.css";

// ---------------------------------------------------------------------------
// Types (mirrors decisions/006 §5)
// ---------------------------------------------------------------------------

interface Message {
  id: string;
  role: "user" | "assistant" | "tool";
  content: string;
  timestamp: string;
  model?: string;
  latency_ms: number;
  tokens: number;
  driver: string;
  participant_name: string;
  reasoning?: string;
  agent?: string;
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let messages: Message[] = [];
let streamingContent: { [streamId: string]: string } = {};
let activeStreamId: string | null = null;
let transcriptHash = "";
/** Message ids whose Thinking block the user has expanded — survives poll re-renders. */
const openReasoningIds = new Set<string>();

function captureOpenReasoning(): void {
  openReasoningIds.clear();
  for (const details of document.querySelectorAll<HTMLDetailsElement>(
    "#transcript .message .reasoning"
  )) {
    if (!details.open) continue;
    const id = details.closest<HTMLElement>(".message")?.dataset.id;
    if (id) openReasoningIds.add(id);
  }
}

function restoreOpenReasoning(): void {
  for (const id of openReasoningIds) {
    const details = document.querySelector<HTMLDetailsElement>(
      `#transcript .message[data-id="${id}"] .reasoning`
    );
    if (details) details.open = true;
  }
}

// ---------------------------------------------------------------------------
// DOM helpers
// ---------------------------------------------------------------------------

function el<T extends HTMLElement>(id: string): T {
  return document.getElementById(id) as T;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function formatTime(iso: string): string {
  return new Date(iso).toLocaleTimeString();
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

function renderTranscript(): void {
  const list = el<HTMLDivElement>("transcript");
  captureOpenReasoning();
  list.innerHTML = "";
  for (const msg of messages) {
    list.appendChild(renderMessage(msg));
  }
  restoreOpenReasoning();
  if (activeStreamId) {
    const streaming = el<HTMLDivElement>("streaming-bubble");
    if (!streaming) {
      const div = renderStreamingBubble(streamingContent[activeStreamId] ?? "");
      div.id = "streaming-bubble";
      list.appendChild(div);
    } else {
      const content = streaming.querySelector(".msg-content");
      if (content) content.textContent = streamingContent[activeStreamId] ?? "";
    }
  }
  list.scrollTop = list.scrollHeight;
}

function renderMessage(msg: Message): HTMLDivElement {
  const div = document.createElement("div");
  div.className = `message message-${msg.role}`;
  div.dataset.id = msg.id;

  const header = document.createElement("div");
  header.className = "msg-header";

  if (msg.role === "assistant") {
    const label = [
      `<span class="participant">Model</span>`,
      msg.model ? `<span class="model-id">${escapeHtml(msg.model)}</span>` : "",
      `<span class="muted">reply to ${escapeHtml(msg.participant_name)}</span>`,
      msg.latency_ms > 0 ? `<span class="stat">${msg.latency_ms}ms</span>` : "",
      msg.tokens > 0 ? `<span class="stat">${msg.tokens} tok</span>` : "",
      `<span class="time">${formatTime(msg.timestamp)}</span>`,
    ]
      .filter(Boolean)
      .join(" · ");
    header.innerHTML = label;
  } else {
    header.innerHTML = `<span class="participant">${escapeHtml(msg.participant_name)}</span> <span class="time">${formatTime(msg.timestamp)}</span>`;
  }

  div.appendChild(header);

  // Collapsible reasoning block
  if (msg.reasoning) {
    const details = document.createElement("details");
    details.className = "reasoning";
    if (openReasoningIds.has(msg.id)) details.open = true;
    details.addEventListener("toggle", () => {
      if (details.open) openReasoningIds.add(msg.id);
      else openReasoningIds.delete(msg.id);
    });
    const summary = document.createElement("summary");
    summary.textContent = "Thinking";
    const pre = document.createElement("pre");
    pre.textContent = msg.reasoning;
    details.appendChild(summary);
    details.appendChild(pre);
    div.appendChild(details);
  }

  const content = document.createElement("div");
  content.className = "msg-content";
  content.textContent = msg.content;
  div.appendChild(content);

  return div;
}

function renderStreamingBubble(partial: string): HTMLDivElement {
  const div = document.createElement("div");
  div.className = "message message-assistant streaming";
  const header = document.createElement("div");
  header.className = "msg-header";
  header.innerHTML = `<span class="participant">Model</span> <span class="muted">typing…</span>`;
  const content = document.createElement("div");
  content.className = "msg-content";
  content.textContent = partial;
  div.appendChild(header);
  div.appendChild(content);
  return div;
}

// ---------------------------------------------------------------------------
// IPC calls
// ---------------------------------------------------------------------------

async function loadTranscript(): Promise<void> {
  try {
    const result = await invoke<{ messages: Message[]; transcript_hash?: string }>(
      "core_invoke",
      {
        id: "transcript:get@1",
        input: {},
      }
    );
    const hash = result.transcript_hash ?? JSON.stringify(result.messages);
    if (hash === transcriptHash && !activeStreamId) return;
    transcriptHash = hash;
    messages = result.messages;
    renderTranscript();
  } catch (e) {
    console.error("transcript:get@1 failed", e);
  }
}

async function connectToLmStudio(): Promise<void> {
  const urlInput = el<HTMLInputElement>("lmstudio-url");
  const url = urlInput.value.trim() || "http://localhost:1234";
  const statusEl = el<HTMLSpanElement>("connection-status");

  statusEl.textContent = "connecting…";
  statusEl.className = "status-connecting";

  try {
    const result = await invoke<{ ok: boolean; model_count: number; models: string[] }>(
      "core_invoke",
      { id: "provider:connect@1", input: { url } }
    );
    statusEl.textContent = `connected · ${result.model_count} model(s)`;
    statusEl.className = "status-ok";

    // Populate model dropdown
    const modelSelect = el<HTMLSelectElement>("model-select");
    modelSelect.innerHTML = "";
    for (const m of result.models) {
      const opt = document.createElement("option");
      opt.value = m;
      opt.textContent = m;
      modelSelect.appendChild(opt);
    }
  } catch (e) {
    statusEl.textContent = `error: ${e}`;
    statusEl.className = "status-error";
  }
}

async function sendMessage(): Promise<void> {
  const input = el<HTMLTextAreaElement>("message-input");
  const text = input.value.trim();
  if (!text) return;

  input.value = "";
  input.disabled = true;

  try {
    const model = el<HTMLSelectElement>("model-select").value || undefined;
    await invoke("core_invoke", {
      id: "transcript:add-user-message@1",
      input: { content: text, observer_name: "human" },
    });
    const transcript = await invoke<{ messages: Message[] }>("core_invoke", {
      id: "transcript:get@1",
      input: {},
    });
    await invoke("core_invoke", {
      id: "provider:generate@1",
      input: { messages: transcript.messages, model },
    });
  } catch (e) {
    console.error("send failed", e);
  } finally {
    input.disabled = false;
    input.focus();
  }
}

// ---------------------------------------------------------------------------
// Event listeners (bus events forwarded by IPC bridge)
// ---------------------------------------------------------------------------

async function wireEvents(): Promise<void> {
  // TODO: When Tauri IPC bridge emits bus events to the frontend via Tauri::emit(),
  // subscribe with listen("transcript://message-added", ...) etc.
  // For Phase 2, we poll via loadTranscript() on a short interval as a fallback.
}

// ---------------------------------------------------------------------------
// Token budget display (decisions/006 §13 — harness cost line item)
// ---------------------------------------------------------------------------

async function updateTokenBudget(): Promise<void> {
  try {
    const result = await invoke<{ prompt: string; token_estimate: number }>("core_invoke", {
      id: "context-editor:system-prompt@1",
      input: {},
    });
    el<HTMLSpanElement>("token-budget").textContent =
      `harness: ~${result.token_estimate} tok`;
  } catch {
    // context-editor may not be loaded in all dev scenarios
  }
}

// ---------------------------------------------------------------------------
// Boot
// ---------------------------------------------------------------------------

async function main(): Promise<void> {
  document.body.innerHTML = `
    <div class="layout">
      <header class="topbar">
        <h1>Nulqor</h1>
        <div class="connection-bar">
          <input id="lmstudio-url" type="text" placeholder="http://localhost:1234" value="http://localhost:1234" />
          <button id="connect-btn">Connect</button>
          <select id="model-select"><option value="">— no models —</option></select>
          <span id="connection-status" class="status-idle">not connected</span>
          <span id="token-budget" class="muted"></span>
        </div>
      </header>
      <main id="transcript" class="transcript"></main>
      <footer class="input-bar">
        <textarea id="message-input" placeholder="Type a message…" rows="3"></textarea>
        <button id="send-btn">Send</button>
      </footer>
    </div>
  `;

  el("connect-btn").addEventListener("click", async () => {
    await connectToLmStudio();
    await updateTokenBudget();
  });

  el("send-btn").addEventListener("click", sendMessage);

  el<HTMLTextAreaElement>("message-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      sendMessage();
    }
  });

  await wireEvents();
  await loadTranscript();

  // Lightweight poll until Tauri event forwarding is fully wired
  setInterval(loadTranscript, 2000);
}

main();
