import { invoke } from "@tauri-apps/api/core";
import "./style.css";

// ---------------------------------------------------------------------------
// Types
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

interface SessionEntry {
  id: string;
  title: string;
  summary: string;
  updated: string;
}

interface RailMarker {
  id: string;
  kind: "auto" | "user";
  type: "human" | "assistant" | "fork" | "bookmark";
  message_id: string;
  fork_id?: string;
  symbol?: string;
  preview: string;
  note?: string;
}

interface ForkRecord {
  id: string;
  label: string;
  message_count: number;
}

const MARKER_SYMBOLS: Record<string, string> = {
  star: "★",
  flag: "⚑",
  question: "?",
  idea: "💡",
  fork: "⎇",
};

const MARKER_GLYPH: Record<string, string> = {
  human: "●",
  assistant: "◆",
  fork: "⎇",
  bookmark: "★",
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

let rootEl: HTMLElement | null = null;
let messages: Message[] = [];
let railMarkers: RailMarker[] = [];
let sessions: SessionEntry[] = [];
let activeSessionId = "";
let transcriptHash = "";
let pollTimer: ReturnType<typeof setInterval> | null = null;
const openReasoningIds = new Set<string>();

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function coreInvoke<T>(id: string, input: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>("core_invoke", { id, input });
}

function el<T extends HTMLElement>(selector: string): T {
  if (!rootEl) throw new Error("chat panel not mounted");
  const node = rootEl.querySelector(selector);
  if (!node) throw new Error(`missing ${selector}`);
  return node as T;
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

function markerGlyph(m: RailMarker): string {
  if (m.symbol && MARKER_SYMBOLS[m.symbol]) return MARKER_SYMBOLS[m.symbol];
  return MARKER_GLYPH[m.type] ?? "·";
}

function captureOpenReasoning(): void {
  openReasoningIds.clear();
  if (!rootEl) return;
  for (const details of rootEl.querySelectorAll<HTMLDetailsElement>(".message .reasoning")) {
    if (!details.open) continue;
    const id = details.closest<HTMLElement>(".message")?.dataset.id;
    if (id) openReasoningIds.add(id);
  }
}

function restoreOpenReasoning(): void {
  if (!rootEl) return;
  for (const id of openReasoningIds) {
    const details = rootEl.querySelector<HTMLDetailsElement>(
      `.message[data-id="${id}"] .reasoning`,
    );
    if (details) details.open = true;
  }
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

async function loadSessions(): Promise<void> {
  try {
    const result = await coreInvoke<{ sessions: SessionEntry[]; active_session_id: string }>(
      "sessions:list@1",
    );
    sessions = result.sessions ?? [];
    activeSessionId = result.active_session_id ?? "";
    const select = el<HTMLSelectElement>("#session-select");
    select.innerHTML = "";
    for (const s of sessions) {
      const opt = document.createElement("option");
      opt.value = s.id;
      const label = s.summary || s.title || s.id;
      opt.textContent = `${label} (${s.id})`;
      if (s.id === activeSessionId) opt.selected = true;
      select.appendChild(opt);
    }
  } catch (e) {
    console.warn("sessions:list@1 unavailable", e);
  }
}

async function loadRail(): Promise<void> {
  try {
    const result = await coreInvoke<{ markers: RailMarker[] }>("human-rail:list@1");
    railMarkers = result.markers ?? [];
    renderRail();
  } catch (e) {
    console.warn("human-rail:list@1 unavailable", e);
  }
}

async function loadTranscript(): Promise<void> {
  try {
    const result = await coreInvoke<{ messages: Message[]; transcript_hash?: string }>(
      "transcript:get@1",
    );
    const hash = result.transcript_hash ?? JSON.stringify(result.messages);
    if (hash === transcriptHash) return;
    transcriptHash = hash;
    messages = result.messages;
    renderTranscript();
  } catch (e) {
    console.error("transcript:get@1 failed", e);
  }
}

async function refreshAll(): Promise<void> {
  await Promise.all([loadTranscript(), loadRail()]);
}

// ---------------------------------------------------------------------------
// Render — main chat (active branch only)
// ---------------------------------------------------------------------------

function renderTranscript(): void {
  const list = el<HTMLDivElement>("#transcript");
  captureOpenReasoning();
  list.innerHTML = "";
  for (const msg of messages) {
    list.appendChild(renderMessage(msg));
  }
  restoreOpenReasoning();
}

function renderMessage(msg: Message): HTMLDivElement {
  const div = document.createElement("div");
  div.className = `message message-${msg.role}`;
  div.dataset.id = msg.id;

  const header = document.createElement("div");
  header.className = "msg-header";

  if (msg.role === "assistant") {
    header.innerHTML = [
      `<span class="participant">Model</span>`,
      msg.model ? `<span class="model-id">${escapeHtml(msg.model)}</span>` : "",
      `<span class="muted">reply to ${escapeHtml(msg.participant_name)}</span>`,
      msg.latency_ms > 0 ? `<span class="stat">${msg.latency_ms}ms</span>` : "",
      msg.tokens > 0 ? `<span class="stat">${msg.tokens} tok</span>` : "",
      `<span class="time">${formatTime(msg.timestamp)}</span>`,
    ]
      .filter(Boolean)
      .join(" · ");
  } else {
    header.innerHTML = `<span class="participant">${escapeHtml(msg.participant_name)}</span> <span class="time">${formatTime(msg.timestamp)}</span> <button type="button" class="msg-edit-btn" data-id="${msg.id}" title="Edit message">Edit</button>`;
  }

  div.appendChild(header);

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
    details.append(summary, pre);
    div.appendChild(details);
  }

  const content = document.createElement("div");
  content.className = "msg-content";
  content.textContent = msg.content;
  div.appendChild(content);

  return div;
}

// ---------------------------------------------------------------------------
// Render — human rail
// ---------------------------------------------------------------------------

function renderRail(): void {
  const rail = el<HTMLDivElement>("#rail-list");
  rail.innerHTML = "";

  if (railMarkers.length === 0) {
    rail.innerHTML = `<div class="rail-empty muted">No markers yet</div>`;
    return;
  }

  for (const m of railMarkers) {
    const row = document.createElement("button");
    row.type = "button";
    row.className = `rail-row rail-${m.type}`;
    row.dataset.messageId = m.message_id;
    if (m.fork_id) row.dataset.forkId = m.fork_id;

    const glyph = document.createElement("span");
    glyph.className = "rail-glyph";
    glyph.textContent = markerGlyph(m);

    const body = document.createElement("span");
    body.className = "rail-body";
    const label =
      m.type === "fork"
        ? m.note ?? "Archived fork"
        : m.preview || m.note || m.type;
    body.textContent = label;

    row.append(glyph, body);
    rail.appendChild(row);
  }
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

function jumpToMessage(messageId: string): void {
  const list = el<HTMLDivElement>("#transcript");
  const target = list.querySelector<HTMLElement>(`.message[data-id="${messageId}"]`);
  if (!target) return;
  target.classList.add("message-highlight");
  target.scrollIntoView({ behavior: "smooth", block: "center" });
  window.setTimeout(() => target.classList.remove("message-highlight"), 1200);
}

function jumpToLatestHuman(): void {
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].role === "user") {
      jumpToMessage(messages[i].id);
      return;
    }
  }
}

async function openForkOverlay(forkId: string): Promise<void> {
  const overlay = el<HTMLDivElement>("#fork-overlay");
  const body = el<HTMLDivElement>("#fork-body");
  body.innerHTML = `<p class="muted">Loading archived fork…</p>`;
  overlay.hidden = false;

  try {
    const result = await coreInvoke<{ messages: Message[]; fork: ForkRecord }>(
      "human-branch:open@1",
      { fork_id: forkId },
    );
    const title = el<HTMLHeadingElement>("#fork-title");
    title.textContent = result.fork?.label ?? "Archived fork";
    body.innerHTML = "";
    for (const msg of result.messages) {
      const block = document.createElement("div");
      block.className = `fork-msg fork-${msg.role}`;
      block.innerHTML = `<div class="fork-msg-head">${escapeHtml(msg.participant_name || msg.role)} · ${formatTime(msg.timestamp)}</div><div class="fork-msg-content">${escapeHtml(msg.content)}</div>`;
      if (msg.reasoning) {
        const thinking = document.createElement("details");
        thinking.className = "reasoning";
        thinking.innerHTML = `<summary>Thinking</summary><pre>${escapeHtml(msg.reasoning)}</pre>`;
        block.appendChild(thinking);
      }
      body.appendChild(block);
    }
  } catch (e) {
    body.innerHTML = `<p class="status-error">${escapeHtml(String(e))}</p>`;
  }
}

function closeForkOverlay(): void {
  el<HTMLDivElement>("#fork-overlay").hidden = true;
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

async function connectToLmStudio(): Promise<void> {
  const url = el<HTMLInputElement>("#lmstudio-url").value.trim() || "http://localhost:1234";
  const statusEl = el<HTMLSpanElement>("#connection-status");
  statusEl.textContent = "connecting…";
  statusEl.className = "status-connecting";

  try {
    const result = await coreInvoke<{ model_count: number; models: string[] }>(
      "provider:connect@1",
      { url },
    );
    statusEl.textContent = `connected · ${result.model_count} model(s)`;
    statusEl.className = "status-ok";
    const modelSelect = el<HTMLSelectElement>("#model-select");
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

async function updateTokenBudget(): Promise<void> {
  try {
    const result = await coreInvoke<{ token_estimate: number }>("context-editor:system-prompt@1");
    el<HTMLSpanElement>("#token-budget").textContent = `harness: ~${result.token_estimate} tok`;
  } catch {
    // context-editor optional in some profiles
  }
}

async function sendMessage(): Promise<void> {
  const input = el<HTMLTextAreaElement>("#message-input");
  const text = input.value.trim();
  if (!text) return;

  input.value = "";
  input.disabled = true;

  try {
    const model = el<HTMLSelectElement>("#model-select").value || undefined;
    await coreInvoke("transcript:add-user-message@1", {
      content: text,
      observer_name: "human",
    });
    const transcript = await coreInvoke<{ messages: Message[] }>("transcript:get@1");
    await coreInvoke("provider:generate@1", {
      messages: transcript.messages,
      model,
    });
    transcriptHash = "";
    await refreshAll();
  } catch (e) {
    console.error("send failed", e);
  } finally {
    input.disabled = false;
    input.focus();
  }
}

async function editUserMessage(messageId: string): Promise<void> {
  const msg = messages.find((m) => m.id === messageId);
  if (!msg || msg.role !== "user") return;

  const next = window.prompt("Edit message (creates archived fork if replies exist):", msg.content);
  if (next === null || next.trim() === msg.content) return;

  try {
    const result = await coreInvoke<{ fork_id?: string; truncated: number }>(
      "sessions:edit-message@1",
      { message_id: messageId, content: next.trim() },
    );
    transcriptHash = "";
    await refreshAll();
    await loadSessions();

    if (result.truncated > 0) {
      const regen = window.confirm(
        `Archived previous branch (${result.truncated} messages). Regenerate assistant reply now?`,
      );
      if (regen) {
        const transcript = await coreInvoke<{ messages: Message[] }>("transcript:get@1");
        const model = el<HTMLSelectElement>("#model-select").value || undefined;
        await coreInvoke("provider:generate@1", {
          messages: transcript.messages,
          model,
        });
        transcriptHash = "";
        await refreshAll();
      }
    }
    jumpToMessage(messageId);
  } catch (e) {
    window.alert(`Edit failed: ${e}`);
  }
}

async function addMarker(messageId: string, symbol: string): Promise<void> {
  try {
    await coreInvoke("human-rail:add-marker@1", { message_id: messageId, symbol });
    await loadRail();
  } catch (e) {
    console.error("add marker failed", e);
  }
}

async function createSession(): Promise<void> {
  const title = window.prompt("New session title:", "New session");
  if (title === null) return;
  await coreInvoke("sessions:create@1", { title });
  transcriptHash = "";
  await loadSessions();
  await refreshAll();
}

async function switchSession(sessionId: string): Promise<void> {
  if (!sessionId || sessionId === activeSessionId) return;
  await coreInvoke("sessions:load@1", { session_id: sessionId });
  activeSessionId = sessionId;
  transcriptHash = "";
  await refreshAll();
}

// ---------------------------------------------------------------------------
// Wire UI
// ---------------------------------------------------------------------------

function wirePanelEvents(): void {
  el("#connect-btn").addEventListener("click", async () => {
    await connectToLmStudio();
    await updateTokenBudget();
  });

  el("#send-btn").addEventListener("click", () => void sendMessage());

  el<HTMLTextAreaElement>("#message-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void sendMessage();
    }
  });

  el("#session-new").addEventListener("click", () => void createSession());

  el<HTMLSelectElement>("#session-select").addEventListener("change", (e) => {
    const id = (e.target as HTMLSelectElement).value;
    void switchSession(id);
  });

  el("#rail-latest-human").addEventListener("click", () => jumpToLatestHuman());

  el("#rail-list").addEventListener("click", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLButtonElement>(".rail-row");
    if (!row) return;
    const forkId = row.dataset.forkId;
    if (forkId) {
      void openForkOverlay(forkId);
      return;
    }
    const messageId = row.dataset.messageId;
    if (messageId) jumpToMessage(messageId);
  });

  el("#transcript").addEventListener("click", (e) => {
    const editBtn = (e.target as HTMLElement).closest<HTMLButtonElement>(".msg-edit-btn");
    if (editBtn?.dataset.id) {
      void editUserMessage(editBtn.dataset.id);
      return;
    }
    const msgEl = (e.target as HTMLElement).closest<HTMLElement>(".message");
    if (msgEl?.dataset.id && e.shiftKey) {
      void addMarker(msgEl.dataset.id, "star");
    }
  });

  el("#fork-close").addEventListener("click", closeForkOverlay);
  el<HTMLDivElement>("#fork-overlay").addEventListener("click", (e) => {
    if (e.target === e.currentTarget) closeForkOverlay();
  });

  el<HTMLSelectElement>("#mark-symbol").addEventListener("change", () => {
    const symbol = el<HTMLSelectElement>("#mark-symbol").value;
    const lastHuman = [...messages].reverse().find((m) => m.role === "user");
    if (lastHuman) void addMarker(lastHuman.id, symbol);
  });
}

function buildShell(container: HTMLElement): void {
  container.classList.add("chat-panel-body");
  container.innerHTML = `
    <div class="chat-layout">
      <header class="topbar">
        <div class="session-bar">
          <select id="session-select" aria-label="Chat session"></select>
          <button type="button" id="session-new" class="btn-secondary">New</button>
        </div>
        <div class="connection-bar">
          <input id="lmstudio-url" type="text" placeholder="http://localhost:1234" value="http://localhost:1234" />
          <button type="button" id="connect-btn">Connect</button>
          <select id="model-select"><option value="">— no models —</option></select>
          <span id="connection-status" class="status-idle">not connected</span>
          <span id="token-budget" class="muted"></span>
        </div>
      </header>
      <div class="chat-split">
        <aside class="rail" aria-label="Conversation map (human only)">
          <div class="rail-toolbar">
            <span class="rail-title">Map</span>
            <button type="button" id="rail-latest-human" class="btn-ghost" title="Jump to your last message">You</button>
            <select id="mark-symbol" title="Mark last user message">
              <option value="">Mark…</option>
              <option value="star">★</option>
              <option value="flag">⚑</option>
              <option value="question">?</option>
              <option value="idea">💡</option>
            </select>
          </div>
          <div id="rail-list" class="rail-list"></div>
        </aside>
        <main id="transcript" class="transcript" aria-label="Active conversation"></main>
      </div>
      <footer class="input-bar">
        <textarea id="message-input" placeholder="Type a message… (Shift+click message to bookmark)" rows="3"></textarea>
        <button type="button" id="send-btn">Send</button>
      </footer>
      <div id="fork-overlay" class="fork-overlay" hidden>
        <div class="fork-dialog" role="dialog" aria-modal="true">
          <header class="fork-header">
            <h2 id="fork-title">Archived fork</h2>
            <p class="muted">Read-only — not sent to the model</p>
            <button type="button" id="fork-close" class="btn-secondary">Close</button>
          </header>
          <div id="fork-body" class="fork-body"></div>
        </div>
      </div>
    </div>
  `;
}

// ---------------------------------------------------------------------------
// Mount
// ---------------------------------------------------------------------------

export function mount(container: HTMLElement): void {
  if (pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }

  rootEl = container;
  buildShell(container);
  wirePanelEvents();

  void (async () => {
    await loadSessions();
    await refreshAll();
    await updateTokenBudget();
    pollTimer = setInterval(() => void refreshAll(), 2000);
  })();
}
