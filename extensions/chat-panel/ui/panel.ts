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

interface ContextProfileAgent {
  name: string;
  path: string;
  excerpt: string;
  enabled: boolean;
}

interface ContextProfileRule {
  filename: string;
  path: string;
  excerpt: string;
  enabled: boolean;
}

interface ContextProfileSkill {
  name: string;
  path: string;
  description: string;
  enabled: boolean;
}

interface ContextProfile {
  session_id?: string;
  active_agent: string;
  token_estimate: number;
  agents: ContextProfileAgent[];
  rules: ContextProfileRule[];
  skills: ContextProfileSkill[];
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
let providerActive: string | null | undefined = undefined;
let providerConnected = false;
let nulqorLoadedActive = false;
let catalogModels: string[] = [];
let activeProviderId = "lmstudio";
let providerDefaultUrl = "http://localhost:1234";
let loadedModelsCache: LoadedModelEntry[] = [];
let contextProfile: ContextProfile | null = null;
let contextProfileLoading = false;
let contextProfileRequestId = 0;
let editingSessionId: string | null = null;
let pendingDeleteSessionId: string | null = null;
let pinTranscriptScroll = true;
let awaitingAssistant = false;
const openReasoningIds = new Set<string>();
const SCROLL_PIN_THRESHOLD = 64;
const MAP_VISIBLE_STORAGE_KEY = "nulqor.chat-panel.map-visible";
const SESSIONS_VISIBLE_STORAGE_KEY = "nulqor.chat-panel.sessions-visible";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async function coreInvoke<T>(
  id: string,
  input: Record<string, unknown> = {},
): Promise<T> {
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

function formatSessionUpdated(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  const now = new Date();
  const sameDay =
    date.getFullYear() === now.getFullYear() &&
    date.getMonth() === now.getMonth() &&
    date.getDate() === now.getDate();
  if (sameDay) return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
  return date.toLocaleDateString([], { month: "short", day: "numeric" });
}

function sessionDisplayTitle(session: SessionEntry): string {
  return session.title.trim() || session.id;
}

function sessionDisplayDescription(session: SessionEntry): string {
  return session.summary.trim();
}

function buildPixelIcon(
  rows: number[][],
  width: number,
  className: string,
  iconWidth = 18,
  iconHeight = 14,
): string {
  const rects = rows.flatMap((xs, y) =>
    xs.map((x) => `<rect x="${x}" y="${y}" width="1" height="1"/>`),
  );
  return `<svg class="${className}" viewBox="0 0 ${width} ${rows.length}" width="${iconWidth}" height="${iconHeight}" aria-hidden="true" focusable="false" shape-rendering="crispEdges"><g fill="currentColor">${rects.join("")}</g></svg>`;
}

const ICON_GEAR = `<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" focusable="false"><path fill="currentColor" d="M8 4.75a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5ZM5.21 8a2.79 2.79 0 1 1 5.58 0 2.79 2.79 0 0 1-5.58 0Z"/><path fill="currentColor" d="M8.73 1.04a.73.73 0 0 0-.73 0l-.62.36a.73.73 0 0 1-.8-.12l-.5-.5a.73.73 0 0 0-1.03 0l-.52.52a.73.73 0 0 0 0 1.03l.5.5a.73.73 0 0 1 .12.8l-.36.62a.73.73 0 0 0 0 .73l.36.62a.73.73 0 0 1-.12.8l-.5.5a.73.73 0 0 0 0 1.03l.52.52a.73.73 0 0 0 1.03 0l.5-.5a.73.73 0 0 1 .8-.12l.62.36a.73.73 0 0 0 .73 0l.62-.36a.73.73 0 0 1 .8.12l.5.5a.73.73 0 0 0 1.03 0l.52-.52a.73.73 0 0 0 0-1.03l-.5-.5a.73.73 0 0 1-.12-.8l.36-.62a.73.73 0 0 0 0-.73l-.36-.62a.73.73 0 0 1 .12-.8l.5-.5a.73.73 0 0 0 0-1.03l-.52-.52a.73.73 0 0 0-1.03 0l-.5.5a.73.73 0 0 1-.8.12l-.62-.36Z"/></svg>`;
const ICON_SPACE_INVADER = buildPixelIcon(
  [
    [2, 3, 8, 9],
    [1, 2, 3, 4, 5, 6, 7, 8],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    [0, 1, 2, 4, 5, 6, 8, 9, 10],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    [1, 2, 3, 4, 5, 6, 7, 8, 9],
    [0, 1, 9, 10],
  ],
  11,
  "icon-space-invader",
);
const ICON_TRASH = `<svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" focusable="false"><path fill="currentColor" d="M5.5 2 6 1h4l.5 1H14v1H2V2h3.5zM3 4h10l-.9 10H3.9L3 4zm2 2v6h1V6H5zm3 0v6h1V6H8z"/></svg>`;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function isTranscriptAtBottom(list: HTMLElement): boolean {
  return (
    list.scrollHeight - list.scrollTop - list.clientHeight <=
    SCROLL_PIN_THRESHOLD
  );
}

function scrollTranscriptToBottom(behavior: ScrollBehavior = "smooth"): void {
  const list = el<HTMLDivElement>("#transcript");
  const apply = () => {
    if (behavior === "auto") {
      list.scrollTop = list.scrollHeight;
    } else {
      list.scrollTo({ top: list.scrollHeight, behavior });
    }
    // Fallback when host tile styles still allow outer scroll.
    if (rootEl && rootEl.scrollHeight > rootEl.clientHeight) {
      rootEl.scrollTop = rootEl.scrollHeight;
    }
  };
  apply();
  requestAnimationFrame(() => {
    apply();
    requestAnimationFrame(apply);
  });
}

function wireTranscriptScroll(): void {
  const list = el<HTMLDivElement>("#transcript");
  list.addEventListener(
    "scroll",
    () => {
      pinTranscriptScroll = isTranscriptAtBottom(list);
    },
    { passive: true },
  );
}

function markerGlyph(m: RailMarker): string {
  if (m.symbol && MARKER_SYMBOLS[m.symbol]) return MARKER_SYMBOLS[m.symbol];
  return MARKER_GLYPH[m.type] ?? "·";
}

function captureOpenReasoning(): void {
  if (!rootEl) return;
  for (const details of rootEl.querySelectorAll<HTMLDetailsElement>(
    ".message .reasoning",
  )) {
    const id = details.closest<HTMLElement>(".message")?.dataset.id;
    if (!id) continue;
    if (!details.open) openReasoningIds.add(`closed:${id}`);
    else openReasoningIds.delete(`closed:${id}`);
  }
}

function restoreOpenReasoning(): void {
  // Applied during renderMessage via closed:* markers.
}

function loadMapVisible(): boolean {
  try {
    if (localStorage.getItem(MAP_VISIBLE_STORAGE_KEY) === "false") return false;
  } catch {
    // ignore storage errors
  }
  return true;
}

function saveMapVisible(visible: boolean): void {
  try {
    localStorage.setItem(MAP_VISIBLE_STORAGE_KEY, String(visible));
  } catch {
    // ignore storage errors
  }
}

function applyMapVisibility(visible: boolean): void {
  const layout = el<HTMLElement>(".chat-layout");
  const btn = el<HTMLButtonElement>("#toggle-map-btn");
  layout.classList.toggle("map-hidden", !visible);
  btn.setAttribute("aria-pressed", String(visible));
  btn.title = visible ? "Hide conversation map" : "Show conversation map";
  btn.textContent = visible ? "Hide map" : "Show map";
}

function toggleMapVisibility(): void {
  const visible = !el<HTMLElement>(".chat-layout").classList.contains("map-hidden");
  applyMapVisibility(!visible);
  saveMapVisible(!visible);
}

function loadSessionsVisible(): boolean {
  try {
    if (localStorage.getItem(SESSIONS_VISIBLE_STORAGE_KEY) === "false") return false;
  } catch {
    // ignore storage errors
  }
  return true;
}

function saveSessionsVisible(visible: boolean): void {
  try {
    localStorage.setItem(SESSIONS_VISIBLE_STORAGE_KEY, String(visible));
  } catch {
    // ignore storage errors
  }
}

function applySessionsVisibility(visible: boolean): void {
  const layout = el<HTMLElement>(".chat-layout");
  const btn = el<HTMLButtonElement>("#toggle-sessions-btn");
  layout.classList.toggle("sessions-hidden", !visible);
  btn.setAttribute("aria-pressed", String(visible));
  btn.title = visible ? "Hide sessions panel" : "Show sessions panel";
  btn.textContent = visible ? "Hide chats" : "Show chats";
}

function toggleSessionsVisibility(): void {
  const visible = !el<HTMLElement>(".chat-layout").classList.contains(
    "sessions-hidden",
  );
  applySessionsVisibility(!visible);
  saveSessionsVisible(!visible);
}

// ---------------------------------------------------------------------------
// Data loading
// ---------------------------------------------------------------------------

async function loadSessions(): Promise<void> {
  try {
    const result = await coreInvoke<{
      sessions: SessionEntry[];
      active_session_id: string;
    }>("sessions:list@1");
    sessions = result.sessions ?? [];
    activeSessionId = result.active_session_id ?? "";
    renderSessionsList();
  } catch (e) {
    console.warn("sessions:list@1 unavailable", e);
  }
}

function renderSessionsList(): void {
  const list = el<HTMLDivElement>("#session-list");
  list.innerHTML = "";

  if (sessions.length === 0) {
    const empty = document.createElement("p");
    empty.className = "session-empty muted";
    empty.textContent = "No sessions yet";
    list.appendChild(empty);
    return;
  }

  for (const session of sessions) {
    const wrap = document.createElement("div");
    wrap.className = "session-row-wrap";
    if (session.id === activeSessionId) wrap.classList.add("is-active");

    const row = document.createElement("button");
    row.type = "button";
    row.className = "session-row";
    row.dataset.sessionId = session.id;
    row.setAttribute(
      "aria-current",
      session.id === activeSessionId ? "true" : "false",
    );

    const title = document.createElement("span");
    title.className = "session-row-title";
    title.textContent = sessionDisplayTitle(session);

    const meta = document.createElement("span");
    meta.className = "session-row-meta";
    const desc = sessionDisplayDescription(session);
    const updated = formatSessionUpdated(session.updated);
    if (desc) {
      meta.textContent = desc;
    } else {
      meta.textContent = updated ? `${updated} · ${session.id}` : session.id;
    }

    row.append(title, meta);

    const actions = document.createElement("div");
    actions.className = "session-row-actions";

    const editBtn = document.createElement("button");
    editBtn.type = "button";
    editBtn.className = "btn-ghost btn-icon session-edit-btn";
    editBtn.dataset.sessionId = session.id;
    editBtn.title = "Chat settings (name, description, agent context)";
    editBtn.setAttribute("aria-label", "Chat settings");
    editBtn.innerHTML = ICON_GEAR;

    const deleteBtn = document.createElement("button");
    deleteBtn.type = "button";
    deleteBtn.className = "btn-ghost btn-icon session-delete-btn";
    deleteBtn.dataset.sessionId = session.id;
    deleteBtn.title = "Delete chat";
    deleteBtn.setAttribute("aria-label", "Delete chat");
    deleteBtn.innerHTML = ICON_TRASH;

    actions.append(editBtn, deleteBtn);
    wrap.append(row, actions);
    list.appendChild(wrap);
  }
}

async function loadRail(): Promise<void> {
  try {
    const result = await coreInvoke<{ markers: RailMarker[] }>(
      "human-rail:list@1",
    );
    railMarkers = result.markers ?? [];
    renderRail();
  } catch (e) {
    console.warn("human-rail:list@1 unavailable", e);
  }
}

async function loadTranscript(forceScroll = false): Promise<void> {
  try {
    const result = await coreInvoke<{
      messages: Message[];
      transcript_hash?: string;
    }>("transcript:get@1");
    const hash = result.transcript_hash ?? JSON.stringify(result.messages);
    if (hash === transcriptHash && !forceScroll) return;
    transcriptHash = hash;
    messages = result.messages;
    renderTranscript(forceScroll);
  } catch (e) {
    console.error("transcript:get@1 failed", e);
  }
}

let refreshInFlight = false;

async function refreshAll(forceTranscriptScroll = false): Promise<void> {
  if (refreshInFlight) return;
  refreshInFlight = true;
  try {
    await Promise.all([
      loadTranscript(forceTranscriptScroll),
      loadRail(),
      loadSessions(),
    ]);
  } finally {
    refreshInFlight = false;
  }
}

// ---------------------------------------------------------------------------
// Render — main chat (active branch only)
// ---------------------------------------------------------------------------

function agentDisplayName(name: string): string {
  return name === "default" ? "Default (AGENTS.md)" : name;
}

function contextSessionId(): string {
  return editingSessionId ?? activeSessionId;
}

async function loadContextProfile(): Promise<ContextProfile | null> {
  const sessionId = contextSessionId();
  if (!sessionId) return null;
  const requestId = ++contextProfileRequestId;
  contextProfileLoading = true;
  try {
    const profile = await coreInvoke<ContextProfile>(
      "context-editor:context-profile@1",
      { session_id: sessionId },
    );
    if (requestId !== contextProfileRequestId) return contextProfile;
    contextProfile = profile;
    el<HTMLSpanElement>("#token-budget").textContent =
      `harness: ~${profile.token_estimate} tok`;
    renderAgentContextPanel(profile);
    return profile;
  } catch (e) {
    if (requestId !== contextProfileRequestId) return contextProfile;
    console.warn("context-editor:context-profile@1 unavailable", e);
    contextProfile = null;
    renderAgentContextPanel(null);
    return null;
  } finally {
    contextProfileLoading = false;
  }
}

async function applyContextProfileUpdate(
  input: Record<string, unknown>,
): Promise<void> {
  const sessionId = contextSessionId();
  if (!sessionId) return;
  try {
    const profile = await coreInvoke<ContextProfile>(
      "context-editor:set-context-profile@1",
      { session_id: sessionId, ...input },
    );
    contextProfile = profile;
    el<HTMLSpanElement>("#token-budget").textContent =
      `harness: ~${profile.token_estimate} tok`;
    renderAgentContextPanel(profile);
  } catch (e) {
    console.error("set-context-profile failed", e);
  }
}

function renderAgentContextPanel(profile: ContextProfile | null): void {
  if (!rootEl) return;
  const agentsEl = rootEl.querySelector("#context-agents-list");
  const rulesEl = rootEl.querySelector("#context-rules-list");
  const skillsEl = rootEl.querySelector("#context-skills-list");
  const summaryEl = rootEl.querySelector("#session-context-summary");
  if (!agentsEl || !rulesEl || !skillsEl || !summaryEl) return;

  if (contextProfileLoading && !profile) {
    summaryEl.textContent = "Loading agent context…";
    agentsEl.innerHTML = "";
    rulesEl.innerHTML = "";
    skillsEl.innerHTML = "";
    return;
  }

  if (!profile) {
    summaryEl.textContent =
      "Context editor unavailable — enable context-editor extension.";
    agentsEl.innerHTML = "";
    rulesEl.innerHTML = "";
    skillsEl.innerHTML = "";
    return;
  }

  const enabledAgents = profile.agents.filter((a) => a.enabled).length;
  const enabledRules = profile.rules.filter((r) => r.enabled).length;
  const enabledSkills = profile.skills.filter((s) => s.enabled).length;
  summaryEl.textContent = `${enabledAgents}/${profile.agents.length} personas · ${enabledRules}/${profile.rules.length} rules · ${enabledSkills}/${profile.skills.length} skills · ~${profile.token_estimate} tok`;

  agentsEl.innerHTML = "";
  if (profile.agents.length === 0) {
    agentsEl.innerHTML = `<li class="context-empty muted">No AGENTS.md or agents/*.md found</li>`;
  } else {
    for (const agent of profile.agents) {
      agentsEl.appendChild(buildContextToggleRow({
        id: agent.name,
        title: agentDisplayName(agent.name),
        path: agent.path,
        detail: agent.excerpt,
        enabled: agent.enabled,
        kind: "agent",
      }));
    }
  }

  rulesEl.innerHTML = "";
  if (profile.rules.length === 0) {
    rulesEl.innerHTML = `<li class="context-empty muted">No rules loaded</li>`;
  } else {
    for (const rule of profile.rules) {
      rulesEl.appendChild(buildContextToggleRow({
        id: rule.filename,
        title: rule.filename,
        path: rule.path,
        detail: rule.excerpt,
        enabled: rule.enabled,
        kind: "rule",
      }));
    }
  }

  skillsEl.innerHTML = "";
  if (profile.skills.length === 0) {
    skillsEl.innerHTML = `<li class="context-empty muted">No skills indexed</li>`;
  } else {
    for (const skill of profile.skills) {
      skillsEl.appendChild(buildContextToggleRow({
        id: skill.name,
        title: skill.name,
        path: skill.path,
        detail: skill.description,
        enabled: skill.enabled,
        kind: "skill",
      }));
    }
  }
}

function buildContextToggleRow(opts: {
  id: string;
  title: string;
  path: string;
  detail: string;
  enabled: boolean;
  kind: "agent" | "rule" | "skill";
}): HTMLLIElement {
  const li = document.createElement("li");
  li.className = "context-item";

  const row = document.createElement("div");
  row.className = "context-toggle-row";

  const header = document.createElement("div");
  header.className = "context-toggle-header";

  const title = document.createElement("span");
  title.className = "context-item-title";
  title.textContent = opts.title;

  const toggleLabel = document.createElement("label");
  toggleLabel.className = "context-toggle-label";
  toggleLabel.title = opts.enabled ? "Included in prompt" : "Excluded from prompt";

  const toggle = document.createElement("input");
  toggle.type = "checkbox";
  toggle.checked = opts.enabled;
  toggle.className = "context-toggle";
  toggle.addEventListener("change", () => {
    const enabled = toggle.checked;
    toggleLabel.title = enabled ? "Included in prompt" : "Excluded from prompt";
    row.classList.toggle("is-disabled", !enabled);
    if (opts.kind === "rule") {
      void applyContextProfileUpdate({
        rule: { filename: opts.id, enabled },
      });
    } else if (opts.kind === "skill") {
      void applyContextProfileUpdate({
        skill: { name: opts.id, enabled },
      });
    } else {
      void applyContextProfileUpdate({
        agent: { name: opts.id, enabled },
      });
    }
  });

  toggleLabel.appendChild(toggle);
  header.append(title, toggleLabel);

  const path = document.createElement("code");
  path.className = "context-item-path";
  path.textContent = opts.path;

  const detail = document.createElement("p");
  detail.className = "context-item-detail muted";
  detail.textContent = opts.detail.trim() || "No summary available.";

  const body = document.createElement("pre");
  body.className = "context-item-body";
  body.hidden = true;

  const readBtn = document.createElement("button");
  readBtn.type = "button";
  readBtn.className = "context-read-btn";
  readBtn.textContent =
    opts.kind === "rule"
      ? "Read rule"
      : opts.kind === "skill"
        ? "Read skill"
        : "Read persona";
  let bodyLoaded = false;
  readBtn.addEventListener("click", (e) => {
    e.preventDefault();
    e.stopPropagation();
    void toggleContextBody(opts, body, readBtn, () => bodyLoaded, (v) => {
      bodyLoaded = v;
    });
  });

  row.classList.toggle("is-disabled", !opts.enabled);
  row.append(header, path, detail, readBtn, body);
  li.appendChild(row);
  return li;
}

async function toggleContextBody(
  opts: { id: string; kind: "agent" | "rule" | "skill" },
  bodyEl: HTMLPreElement,
  readBtn: HTMLButtonElement,
  isLoaded: () => boolean,
  setLoaded: (v: boolean) => void,
): Promise<void> {
  if (!bodyEl.hidden) {
    bodyEl.hidden = true;
    readBtn.textContent =
      opts.kind === "rule"
        ? "Read rule"
        : opts.kind === "skill"
          ? "Read skill"
          : "Read persona";
    readBtn.setAttribute("aria-expanded", "false");
    return;
  }

  if (!isLoaded()) {
    readBtn.disabled = true;
    readBtn.textContent = "Loading…";
    try {
      const content =
        opts.kind === "rule"
          ? await coreInvoke<{ body: string }>("context-editor:load-rule@1", {
              filename: opts.id,
            })
          : opts.kind === "skill"
            ? await coreInvoke<{ body: string }>("context-editor:load-skill@1", {
                name: opts.id,
              })
            : await coreInvoke<{ body: string }>("context-editor:load-agent@1", {
                name: opts.id,
              });
      bodyEl.textContent = content.body;
      setLoaded(true);
    } catch (e) {
      console.error("load context body failed", e);
      bodyEl.textContent = `Could not load content: ${e}`;
      setLoaded(true);
    } finally {
      readBtn.disabled = false;
    }
  }

  bodyEl.hidden = false;
  readBtn.textContent = "Hide";
  readBtn.setAttribute("aria-expanded", "true");
}

function renderEmptyTranscriptHint(): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "transcript-empty-hint";
  wrap.innerHTML = `
    <p class="transcript-empty-title">New conversation</p>
    <p class="transcript-empty-lead muted">Connect a model, then send a message to start.</p>
  `;
  return wrap;
}

function renderTranscript(forceScroll = false): void {
  const list = el<HTMLDivElement>("#transcript");
  const isEmpty = messages.length === 0 && !awaitingAssistant;
  const shouldStick =
    !isEmpty &&
    (forceScroll || pinTranscriptScroll || isTranscriptAtBottom(list));
  captureOpenReasoning();
  list.innerHTML = "";
  if (isEmpty) {
    list.appendChild(renderEmptyTranscriptHint());
    list.classList.add("transcript-empty");
    return;
  }
  list.classList.remove("transcript-empty");
  for (const msg of messages) {
    list.appendChild(renderMessage(msg));
  }
  if (awaitingAssistant) {
    const pending = document.createElement("div");
    pending.className = "message message-assistant message-pending";
    pending.innerHTML =
      '<div class="msg-header"><span class="participant">Model</span> <span class="muted">thinking…</span></div><div class="msg-content pending-dots">…</div>';
    list.appendChild(pending);
  }
  restoreOpenReasoning();
  if (shouldStick) {
    requestAnimationFrame(() =>
      scrollTranscriptToBottom(
        forceScroll || awaitingAssistant ? "auto" : "smooth",
      ),
    );
  }
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
    details.open = !openReasoningIds.has(`closed:${msg.id}`);
    details.addEventListener("toggle", () => {
      if (details.open) openReasoningIds.delete(`closed:${msg.id}`);
      else openReasoningIds.add(`closed:${msg.id}`);
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
        ? (m.note ?? "Archived fork")
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
  const target = list.querySelector<HTMLElement>(
    `.message[data-id="${messageId}"]`,
  );
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

interface ProviderEntry {
  id: string;
  label: string;
  default_url: string;
  vram_hint: string;
  enabled: boolean;
}

interface LoadedModelEntry {
  model: string;
  nulqor_owned: boolean;
  instance_id?: string | null;
  ejectable: boolean;
}

const EJECT_ICON_SVG = `<svg class="icon-eject" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true" focusable="false"><path fill="currentColor" d="M8 1.5 3.5 6h2.75V11h3.5V6H12.5L8 1.5ZM4 12.25v1h8v-1H4Z"/></svg>`;

// ---------------------------------------------------------------------------
// Provider connection
// ---------------------------------------------------------------------------

function providerServerUrl(): string {
  return (
    el<HTMLInputElement>("#provider-url").value.trim() || providerDefaultUrl
  );
}

async function loadProviderInfo(): Promise<void> {
  try {
    const info = await coreInvoke<{
      active: string;
      providers: ProviderEntry[];
      available: string[];
    }>("provider:info@1");
    activeProviderId = info.active;
    const activeMeta =
      info.providers.find((p) => p.id === info.active) ?? info.providers[0];
    if (activeMeta) {
      providerDefaultUrl = activeMeta.default_url;
      el<HTMLInputElement>("#provider-url").placeholder = activeMeta.default_url;
      if (
        !el<HTMLInputElement>("#provider-url").value ||
        el<HTMLInputElement>("#provider-url").value === "http://localhost:1234"
      ) {
        el<HTMLInputElement>("#provider-url").value = activeMeta.default_url;
      }
      const hintEl = el<HTMLSpanElement>("#provider-vram-hint");
      hintEl.textContent = activeMeta.vram_hint;
      hintEl.title = activeMeta.vram_hint;
    }

    const select = el<HTMLSelectElement>("#provider-select");
    select.innerHTML = "";
    for (const p of info.providers) {
      if (!p.enabled) continue;
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = p.label;
      opt.title = p.vram_hint;
      select.appendChild(opt);
    }
    select.value = info.active;
    select.disabled = info.available.length <= 1;
  } catch (e) {
    console.warn("[chat-panel] provider:info unavailable:", e);
  }
}

async function switchActiveProvider(providerId: string): Promise<void> {
  if (providerId === activeProviderId) return;

  if (providerConnected) {
    await coreInvoke("provider:disconnect@1").catch(() => undefined);
    providerConnected = false;
    providerActive = null;
    nulqorLoadedActive = false;
    catalogModels = [];
  }

  const result = await coreInvoke<{ active: string; default_url: string }>(
    "provider:set-active@1",
    { provider: providerId },
  );
  activeProviderId = result.active;
  providerDefaultUrl = result.default_url;
  el<HTMLInputElement>("#provider-url").value = result.default_url;
  el<HTMLInputElement>("#provider-url").placeholder = result.default_url;
  populateModelSelect([], null);
  applyProviderUi(false, null, false);
  await loadProviderInfo();
  void refreshLoadedModelsList();
}

function openProviderConfig(): void {
  const overlay = el<HTMLDivElement>("#provider-config-overlay");
  overlay.hidden = false;
  overlay.focus();
  void refreshLoadedModelsList();
}

function closeProviderConfig(): void {
  el<HTMLDivElement>("#provider-config-overlay").hidden = true;
}

function setLoadEjectButton(loaded: boolean): void {
  const btn = el<HTMLButtonElement>("#load-eject-btn");
  btn.textContent = loaded ? "Eject" : "Load";
  btn.dataset.loaded = loaded ? "true" : "false";
  btn.classList.toggle("btn-eject", loaded);
  btn.title = loaded
    ? "Unload model from VRAM (Nulqor-owned only)"
    : "Load selected model into VRAM";
}

function setConnectButtonConnected(connected: boolean): void {
  const connectBtn = el<HTMLButtonElement>("#connect-btn");
  connectBtn.textContent = connected ? "Disconnect" : "Connect";
  connectBtn.dataset.connected = connected ? "true" : "false";
  connectBtn.classList.toggle("btn-disconnect", connected);
}

function lmStudioUrl(): string {
  return providerServerUrl();
}

function mergeCatalogWithLoaded(catalog: string[]): string[] {
  const merged = [...catalog];
  for (const entry of loadedModelsCache) {
    if (!merged.includes(entry.model)) merged.push(entry.model);
  }
  return merged.sort((a, b) => a.localeCompare(b));
}

function renderLoadedModels(loaded: LoadedModelEntry[]): void {
  const listEl = el<HTMLUListElement>("#loaded-models-list");
  const active = providerActive ?? el<HTMLSelectElement>("#model-select").value;
  listEl.innerHTML = "";

  if (loaded.length === 0) {
    const li = document.createElement("li");
    li.className = "loaded-models-empty muted";
    li.textContent = "No models loaded in VRAM";
    listEl.appendChild(li);
    return;
  }

  for (const entry of loaded) {
    const li = document.createElement("li");
    li.className = "loaded-model-item";

    const row = document.createElement("button");
    row.type = "button";
    row.className = "loaded-model-row";
    if (entry.model === active) row.classList.add("active");
    row.dataset.model = entry.model;
    row.title = `Use ${entry.model}`;
    row.setAttribute("aria-label", `Use ${entry.model}`);

    const name = document.createElement("span");
    name.className = "loaded-model-name";
    name.textContent = entry.model;

    const badge = document.createElement("span");
    badge.className = entry.nulqor_owned
      ? "loaded-model-badge owned"
      : "loaded-model-badge external";
    badge.textContent = entry.nulqor_owned ? "Nulqor" : "external";

    row.append(name, badge);

    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "btn-icon btn-eject-icon";
    btn.dataset.model = entry.model;
    btn.disabled = !entry.ejectable;
    btn.title = entry.ejectable
      ? "Eject from VRAM"
      : "Cannot eject — restart the server";
    btn.setAttribute("aria-label", `Eject ${entry.model}`);
    btn.innerHTML = EJECT_ICON_SVG;

    li.append(row, btn);
    listEl.appendChild(li);
  }
}

async function refreshLoadedModelsList(): Promise<void> {
  if (document.getElementById("loaded-models-list") === null) return;
  try {
    const result = await coreInvoke<{ loaded: LoadedModelEntry[] }>(
      "provider:loaded-models@1",
      { refresh: true, url: lmStudioUrl() },
    );
    loadedModelsCache = result.loaded;
    renderLoadedModels(loadedModelsCache);
    if (catalogModels.length > 0) {
      populateModelSelect(
        catalogModels,
        providerActive ??
          (el<HTMLSelectElement>("#model-select").value || null),
      );
    }
  } catch {
    loadedModelsCache = [];
    renderLoadedModels([]);
  }
}

async function activateModel(model: string): Promise<void> {
  if (!model) return;

  const statusEl = el<HTMLSpanElement>("#connection-status");
  const isLoaded = loadedModelsCache.some((entry) => entry.model === model);

  el<HTMLSelectElement>("#model-select").value = model;
  providerActive = model;
  renderLoadedModels(loadedModelsCache);

  if (!providerConnected && !isLoaded) {
    statusEl.textContent = `selected · ${model}`;
    statusEl.className = "status-idle";
    updateModelActionButtons();
    return;
  }

  statusEl.textContent = `activating ${model}…`;
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    await coreInvoke("provider:models@1", { refresh: true, url: lmStudioUrl() });
    const result = await coreInvoke<{
      active: string;
      nulqor_loaded: boolean;
    }>("provider:select-model@1", { model });
    populateModelSelect(catalogModels, result.active);
    applyProviderUi(true, result.active, result.nulqor_loaded);
    await refreshLoadedModelsList();
  } catch (e) {
    statusEl.textContent = `activate failed: ${e}`;
    statusEl.className = "status-error";
  } finally {
    updateModelActionButtons();
  }
}

async function ejectLoadedModel(model: string): Promise<void> {
  const statusEl = el<HTMLSpanElement>("#connection-status");
  statusEl.textContent = `ejecting ${model}…`;
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    const result = await coreInvoke<{ stopped: boolean }>(
      "provider:unload-model@1",
      { model },
    );
    await syncConnectionFromProvider(false);
    await refreshLoadedModelsList();
    if (result.stopped) {
      if (providerActive === model) {
        applyProviderUi(false, null, false);
        populateModelSelect(catalogModels, null);
      }
      nulqorLoadedActive = false;
      setLoadEjectButton(false);
      statusEl.textContent = `ejected · ${model}`;
      statusEl.className = "status-idle";
    } else {
      statusEl.textContent = `could not eject ${model}`;
      statusEl.className = "status-error";
    }
  } catch (e) {
    statusEl.textContent = `eject failed: ${e}`;
    statusEl.className = "status-error";
  } finally {
    updateModelActionButtons();
  }
}

function updateModelActionButtons(): void {
  const fetchBtn = el<HTMLButtonElement>("#fetch-models-btn");
  const loadEjectBtn = el<HTMLButtonElement>("#load-eject-btn");
  const connectBtn = el<HTMLButtonElement>("#connect-btn");
  const modelSelect = el<HTMLSelectElement>("#model-select");
  const statusEl = el<HTMLSpanElement>("#connection-status");
  const connecting = statusEl.classList.contains("status-connecting");

  fetchBtn.disabled = connecting;
  modelSelect.disabled = connecting || catalogModels.length === 0;
  const activeModel = providerActive ?? modelSelect.value;
  const activeLoaded = loadedModelsCache.find(
    (entry) => entry.model === activeModel,
  );
  const canEject = Boolean(activeLoaded?.nulqor_owned);
  setLoadEjectButton(canEject);
  if (connecting) {
    loadEjectBtn.disabled = true;
    connectBtn.disabled = true;
  } else {
    loadEjectBtn.disabled = canEject ? false : !modelSelect.value;
    connectBtn.disabled = false;
  }
}

function populateModelSelect(models: string[], active: string | null): void {
  const modelSelect = el<HTMLSelectElement>("#model-select");
  catalogModels = models;
  const displayModels = mergeCatalogWithLoaded(models);
  modelSelect.innerHTML = "";

  if (displayModels.length === 0) {
    const opt = document.createElement("option");
    opt.value = "";
    opt.textContent = "— fetch models —";
    modelSelect.appendChild(opt);
    modelSelect.disabled = true;
    updateModelActionButtons();
    return;
  }

  if (!active) {
    const placeholder = document.createElement("option");
    placeholder.value = "";
    placeholder.textContent = "— select model —";
    modelSelect.appendChild(placeholder);
  }

  for (const m of displayModels) {
    const opt = document.createElement("option");
    opt.value = m;
    opt.textContent = m;
    modelSelect.appendChild(opt);
  }

  const selected = active ?? providerActive;
  if (selected) modelSelect.value = selected;
  renderLoadedModels(loadedModelsCache);
  updateModelActionButtons();
  if (messages.length === 0 && !awaitingAssistant) {
    renderTranscript();
  }
}

function applyProviderUi(
  connected: boolean,
  active: string | null,
  ownedActive = nulqorLoadedActive,
): void {
  const statusEl = el<HTMLSpanElement>("#connection-status");
  providerConnected = connected;
  providerActive = active;
  nulqorLoadedActive = ownedActive;

  if (!connected) {
    const selected =
      active ?? (el<HTMLSelectElement>("#model-select").value || null);
    if (selected) providerActive = selected;
    statusEl.textContent = selected
      ? `selected · ${selected}`
      : catalogModels.length > 0
        ? "configure model, then connect"
        : "not connected";
    statusEl.className = "status-idle";
    setConnectButtonConnected(false);
    renderLoadedModels(loadedModelsCache);
    updateModelActionButtons();
    if (messages.length === 0 && !awaitingAssistant) {
      renderTranscript();
    }
    return;
  }

  setConnectButtonConnected(true);
  if (active) {
    statusEl.textContent = ownedActive
      ? `ready · ${active}`
      : `using external · ${active}`;
    statusEl.className = "status-ok";
  } else {
    statusEl.textContent = "configure model in config, then connect";
    statusEl.className = "status-idle";
  }
  updateModelActionButtons();
  if (messages.length === 0 && !awaitingAssistant) {
    renderTranscript();
  }
}

async function fetchModelsFromProvider(): Promise<void> {
  const statusEl = el<HTMLSpanElement>("#connection-status");
  statusEl.textContent = "fetching models…";
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    const result = await coreInvoke<{
      models: string[];
      active: string | null;
      connected: boolean;
      nulqor_loaded_active: boolean;
    }>("provider:models@1", { refresh: true, url: lmStudioUrl() });
    populateModelSelect(result.models, providerConnected ? result.active : null);
    applyProviderUi(
      providerConnected,
      providerConnected ? result.active : null,
      result.nulqor_loaded_active,
    );
    if (result.models.length === 0) {
      statusEl.textContent = "no models found";
      statusEl.className = "status-error";
    } else {
      statusEl.textContent = `${result.models.length} models — pick one in config`;
      statusEl.className = "status-idle";
    }
    await refreshLoadedModelsList();
  } catch (e) {
    statusEl.textContent = `fetch failed: ${e}`;
    statusEl.className = "status-error";
  } finally {
    updateModelActionButtons();
  }
}

async function loadSelectedModel(): Promise<void> {
  const url = lmStudioUrl();
  const model = el<HTMLSelectElement>("#model-select").value;
  const statusEl = el<HTMLSpanElement>("#connection-status");

  if (!model) {
    statusEl.textContent = "select a model to load";
    statusEl.className = "status-error";
    return;
  }

  statusEl.textContent = `loading ${model}…`;
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    await coreInvoke("provider:models@1", { refresh: true, url });
    const result = await coreInvoke<{
      active: string;
      nulqor_loaded: boolean;
    }>("provider:select-model@1", { model });
    populateModelSelect(catalogModels, result.active);
    applyProviderUi(true, result.active, result.nulqor_loaded);
    statusEl.textContent = result.nulqor_loaded
      ? `loaded · ${result.active}`
      : `using external · ${result.active}`;
    statusEl.className = "status-ok";
    await refreshLoadedModelsList();
  } catch (e) {
    statusEl.textContent = `load failed: ${e}`;
    statusEl.className = "status-error";
  } finally {
    updateModelActionButtons();
  }
}

async function ejectModel(): Promise<void> {
  const statusEl = el<HTMLSpanElement>("#connection-status");
  statusEl.textContent = "ejecting model…";
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    const result = await coreInvoke<{
      stopped: boolean;
      active: string | null;
    }>("provider:stop-model@1");
    await syncConnectionFromProvider(false);
    if (result.stopped) {
      applyProviderUi(false, null, false);
      populateModelSelect(catalogModels, null);
      statusEl.textContent = "model ejected";
      statusEl.className = "status-idle";
    } else {
      statusEl.textContent = "nothing to eject (not loaded by Nulqor)";
      statusEl.className = "status-error";
    }
    await refreshLoadedModelsList();
  } catch (e) {
    statusEl.textContent = `eject failed: ${e}`;
    statusEl.className = "status-error";
  } finally {
    updateModelActionButtons();
  }
}

async function toggleLoadEject(): Promise<void> {
  const loaded =
    el<HTMLButtonElement>("#load-eject-btn").dataset.loaded === "true";
  if (loaded) await ejectModel();
  else await loadSelectedModel();
}

async function syncConnectionFromProvider(force = false): Promise<void> {
  const statusEl = el<HTMLSpanElement>("#connection-status");

  if (!force && statusEl.classList.contains("status-connecting")) return;

  try {
    const result = await coreInvoke<{
      models: string[];
      active: string | null;
      connected: boolean;
      nulqor_loaded_active: boolean;
    }>("provider:models@1", { refresh: force });

    populateModelSelect(result.models, result.active);
    applyProviderUi(
      result.connected,
      result.active,
      result.nulqor_loaded_active,
    );
  } catch {
    providerActive = null;
    providerConnected = false;
    nulqorLoadedActive = false;
    populateModelSelect([], null);
    applyProviderUi(false, null, false);
  }
}

function isProviderReadyToConnect(): boolean {
  const model = el<HTMLSelectElement>("#model-select").value;
  return Boolean(model) && catalogModels.length > 0;
}

async function connectToLmStudio(): Promise<void> {
  const url = lmStudioUrl();
  const model = el<HTMLSelectElement>("#model-select").value;
  const statusEl = el<HTMLSpanElement>("#connection-status");

  if (!isProviderReadyToConnect()) {
    statusEl.textContent =
      catalogModels.length === 0
        ? "open config and fetch a model"
        : "open config and select a model";
    statusEl.className = "status-error";
    openProviderConfig();
    return;
  }

  statusEl.textContent = `starting ${model}…`;
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    const result = await coreInvoke<{
      connected: boolean;
      active: string;
      nulqor_loaded: boolean;
    }>("provider:connect@1", { url, model });
    populateModelSelect(catalogModels, result.active);
    applyProviderUi(true, result.active, result.nulqor_loaded);
  } catch (e) {
    statusEl.textContent = `error: ${e}`;
    statusEl.className = "status-error";
    setConnectButtonConnected(false);
    providerActive = null;
    providerConnected = false;
    nulqorLoadedActive = false;
  } finally {
    updateModelActionButtons();
  }
}

async function disconnectFromLmStudio(): Promise<void> {
  const statusEl = el<HTMLSpanElement>("#connection-status");
  statusEl.textContent = "disconnecting…";
  statusEl.className = "status-connecting";
  updateModelActionButtons();

  try {
    await coreInvoke("provider:disconnect@1");
    applyProviderUi(false, null, false);
    populateModelSelect(catalogModels, null);
    statusEl.textContent = "disconnected";
    statusEl.className = "status-idle";
  } catch (e) {
    statusEl.textContent = `error: ${e}`;
    statusEl.className = "status-error";
  } finally {
    updateModelActionButtons();
  }
}

async function toggleProviderConnection(): Promise<void> {
  const connected =
    el<HTMLButtonElement>("#connect-btn").dataset.connected === "true";
  if (connected) {
    await disconnectFromLmStudio();
    return;
  }
  await connectToLmStudio();
  await updateTokenBudget();
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

async function updateTokenBudget(): Promise<void> {
  await loadContextProfile();
}

async function waitForAssistantReply(
  beforeCount: number,
  timeoutMs = 120_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  pinTranscriptScroll = true;
  awaitingAssistant = true;
  renderTranscript();

  while (Date.now() < deadline) {
    await sleep(400);
    // Keep the transcript hash so loadTranscript only re-renders when the
    // server transcript actually changes (e.g. the assistant turn lands).
    await loadTranscript();
    if (
      messages.length > beforeCount &&
      messages[messages.length - 1]?.role === "assistant"
    ) {
      break;
    }
  }

  awaitingAssistant = false;
  transcriptHash = "";
  await loadTranscript();
  scrollTranscriptToBottom("smooth");
}

async function sendMessage(): Promise<void> {
  const input = el<HTMLTextAreaElement>("#message-input");
  const text = input.value.trim();
  if (!text) return;

  input.value = "";
  input.disabled = true;
  pinTranscriptScroll = true;

  try {
    const model =
      el<HTMLSelectElement>("#model-select").value || providerActive || undefined;
    if (!model) {
      input.value = text;
      const statusEl = el<HTMLSpanElement>("#connection-status");
      statusEl.textContent = providerConnected
        ? "no active model"
        : "connect (load model) before sending";
      statusEl.className = "status-error";
      return;
    }
    if (!providerConnected) {
      input.value = text;
      el<HTMLSpanElement>("#connection-status").textContent =
        "connect to load the selected model before sending";
      el<HTMLSpanElement>("#connection-status").className = "status-error";
      return;
    }

    const beforeCount = messages.length;
    await coreInvoke("transcript:add-user-message@1", {
      content: text,
      observer_name: "human",
    });
    transcriptHash = "";
    await loadTranscript();
    scrollTranscriptToBottom("smooth");

    const transcript = await coreInvoke<{ messages: Message[] }>(
      "transcript:get@1",
    );
    await coreInvoke("provider:generate@1", {
      messages: transcript.messages,
      model,
    });
    await waitForAssistantReply(beforeCount);
    await syncConnectionFromProvider(false);
    await loadRail();
  } catch (e) {
    awaitingAssistant = false;
    console.error("send failed", e);
    transcriptHash = "";
    await loadTranscript();
  } finally {
    input.disabled = false;
    input.focus();
  }
}

async function editUserMessage(messageId: string): Promise<void> {
  const msg = messages.find((m) => m.id === messageId);
  if (!msg || msg.role !== "user") return;

  const next = window.prompt(
    "Edit message (creates archived fork if replies exist):",
    msg.content,
  );
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
        const transcript = await coreInvoke<{ messages: Message[] }>(
          "transcript:get@1",
        );
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
    await coreInvoke("human-rail:add-marker@1", {
      message_id: messageId,
      symbol,
    });
    await loadRail();
  } catch (e) {
    console.error("add marker failed", e);
  }
}

async function createSession(): Promise<void> {
  await coreInvoke("sessions:create@1", { title: "New chat" });
  transcriptHash = "";
  await loadSessions();
  await refreshAll(true);
  if (activeSessionId) {
    openSessionEditModal(activeSessionId);
  }
}

function isSessionEditOpen(): boolean {
  return !el<HTMLDivElement>("#session-edit-overlay").hidden;
}

function openSessionEditModal(sessionId: string): void {
  const session = sessions.find((s) => s.id === sessionId);
  if (!session) return;
  const wasHidden = el<HTMLDivElement>("#session-edit-overlay").hidden;
  editingSessionId = sessionId;
  el<HTMLInputElement>("#session-edit-title").value = sessionDisplayTitle(session);
  el<HTMLTextAreaElement>("#session-edit-summary").value =
    sessionDisplayDescription(session);
  el<HTMLDivElement>("#session-edit-overlay").hidden = false;
  el<HTMLDivElement>("#session-edit-overlay").focus();
  void loadContextProfile();
  if (wasHidden) {
    el<HTMLInputElement>("#session-edit-title").focus();
  }
}

function closeSessionEditModal(): void {
  editingSessionId = null;
  el<HTMLDivElement>("#session-edit-overlay").hidden = true;
}

async function saveSessionEdit(): Promise<void> {
  if (!editingSessionId) return;
  const title = el<HTMLInputElement>("#session-edit-title").value.trim();
  const summary = el<HTMLTextAreaElement>("#session-edit-summary").value.trim();
  if (!title) {
    window.alert("Chat name cannot be empty.");
    return;
  }
  try {
    await coreInvoke("sessions:update@1", {
      session_id: editingSessionId,
      title,
      summary,
    });
    closeSessionEditModal();
    await loadSessions();
    if (contextProfile) {
      renderAgentContextPanel(contextProfile);
    }
  } catch (e) {
    window.alert(`Could not save chat: ${e}`);
  }
}

function openSessionDeleteModal(sessionId: string): void {
  const session = sessions.find((s) => s.id === sessionId);
  const label = session ? sessionDisplayTitle(session) : sessionId;
  pendingDeleteSessionId = sessionId;
  el<HTMLParagraphElement>("#session-delete-message").innerHTML =
    `Delete <strong>${escapeHtml(label)}</strong>?`;
  el<HTMLParagraphElement>("#session-delete-error").hidden = true;
  el<HTMLParagraphElement>("#session-delete-error").textContent = "";
  el<HTMLDivElement>("#session-delete-overlay").hidden = false;
  el<HTMLButtonElement>("#session-delete-confirm").focus();
}

function closeSessionDeleteModal(): void {
  pendingDeleteSessionId = null;
  el<HTMLDivElement>("#session-delete-overlay").hidden = true;
}

async function confirmSessionDelete(): Promise<void> {
  if (!pendingDeleteSessionId) return;
  const sessionId = pendingDeleteSessionId;
  const confirmBtn = el<HTMLButtonElement>("#session-delete-confirm");
  const cancelBtn = el<HTMLButtonElement>("#session-delete-cancel");
  const errorEl = el<HTMLParagraphElement>("#session-delete-error");
  confirmBtn.disabled = true;
  cancelBtn.disabled = true;
  errorEl.hidden = true;
  try {
    const result = await coreInvoke<{
      active_session_id: string;
    }>("sessions:delete@1", { session_id: sessionId });
    closeSessionDeleteModal();
    activeSessionId = result.active_session_id;
    transcriptHash = "";
    await loadSessions();
    await refreshAll(true);
    if (contextProfile) {
      renderAgentContextPanel(contextProfile);
    }
  } catch (e) {
    errorEl.textContent = `Could not delete chat: ${e}`;
    errorEl.hidden = false;
  } finally {
    confirmBtn.disabled = false;
    cancelBtn.disabled = false;
  }
}

async function switchSession(sessionId: string): Promise<void> {
  if (!sessionId) return;
  const settingsOpen = isSessionEditOpen();
  if (sessionId === activeSessionId && !settingsOpen) return;
  if (
    sessionId === activeSessionId &&
    settingsOpen &&
    editingSessionId === sessionId
  ) {
    return;
  }

  if (sessionId !== activeSessionId) {
    await coreInvoke("sessions:load@1", { session_id: sessionId });
    activeSessionId = sessionId;
    transcriptHash = "";
    pinTranscriptScroll = true;
    renderSessionsList();
    await refreshAll(true);
  }

  if (settingsOpen) {
    openSessionEditModal(sessionId);
  } else {
    await loadContextProfile();
  }
}

// ---------------------------------------------------------------------------
// Wire UI
// ---------------------------------------------------------------------------

function wirePanelEvents(): void {
  el("#provider-config-btn").addEventListener("click", () => openProviderConfig());

  el("#connect-btn").addEventListener(
    "click",
    () => void toggleProviderConnection(),
  );

  el("#send-btn").addEventListener("click", () => void sendMessage());

  el<HTMLTextAreaElement>("#message-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void sendMessage();
    }
  });

  el("#session-new").addEventListener("click", () => void createSession());

  el("#toggle-map-btn").addEventListener("click", () => toggleMapVisibility());

  el("#fetch-models-btn").addEventListener("click", () =>
    void fetchModelsFromProvider(),
  );

  el<HTMLSelectElement>("#provider-select").addEventListener("change", (e) => {
    const id = (e.target as HTMLSelectElement).value;
    void switchActiveProvider(id).catch((err) => {
      el<HTMLSpanElement>("#connection-status").textContent = `provider: ${err}`;
      el<HTMLSpanElement>("#connection-status").className = "status-error";
    });
  });

  el("#load-eject-btn").addEventListener("click", () => void toggleLoadEject());

  el("#loaded-models-list").addEventListener("click", (e) => {
    const ejectBtn = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".btn-eject-icon",
    );
    if (ejectBtn?.dataset.model && !ejectBtn.disabled) {
      void ejectLoadedModel(ejectBtn.dataset.model);
      return;
    }
    const row = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".loaded-model-row",
    );
    if (!row?.dataset.model) return;
    void activateModel(row.dataset.model);
  });

  el("#provider-config-close").addEventListener("click", closeProviderConfig);
  el<HTMLDivElement>("#provider-config-overlay").addEventListener("click", (e) => {
    if ((e.target as HTMLElement).closest(".provider-config-dialog")) return;
    closeProviderConfig();
  });
  el<HTMLDivElement>("#provider-config-overlay").addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeProviderConfig();
  });

  el<HTMLSelectElement>("#model-select").addEventListener("change", () => {
    const model = el<HTMLSelectElement>("#model-select").value;
    if (model) void activateModel(model);
    else updateModelActionButtons();
  });

  el("#toggle-sessions-btn").addEventListener("click", () =>
    toggleSessionsVisibility(),
  );

  el("#session-list").addEventListener("click", (e) => {
    const editBtn = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".session-edit-btn",
    );
    if (editBtn?.dataset.sessionId) {
      e.preventDefault();
      e.stopPropagation();
      openSessionEditModal(editBtn.dataset.sessionId);
      return;
    }
    const deleteBtn = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".session-delete-btn",
    );
    if (deleteBtn?.dataset.sessionId) {
      e.preventDefault();
      e.stopPropagation();
      void openSessionDeleteModal(deleteBtn.dataset.sessionId);
      return;
    }
    const row = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".session-row",
    );
    if (!row?.dataset.sessionId) return;
    void switchSession(row.dataset.sessionId);
  });

  el("#session-edit-save").addEventListener("click", () => void saveSessionEdit());
  el("#session-edit-cancel").addEventListener("click", closeSessionEditModal);
  el<HTMLDivElement>("#session-edit-overlay").addEventListener("click", (e) => {
    if (e.target === e.currentTarget) closeSessionEditModal();
  });
  el<HTMLDivElement>("#session-edit-overlay").addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeSessionEditModal();
  });

  el("#session-delete-cancel").addEventListener("click", closeSessionDeleteModal);
  el("#session-delete-confirm").addEventListener("click", () =>
    void confirmSessionDelete(),
  );
  el<HTMLDivElement>("#session-delete-overlay").addEventListener("click", (e) => {
    if (e.target === e.currentTarget) closeSessionDeleteModal();
  });
  el<HTMLDivElement>("#session-delete-overlay").addEventListener("keydown", (e) => {
    if (e.key === "Escape") closeSessionDeleteModal();
  });

  el("#rail-latest-human").addEventListener("click", () => jumpToLatestHuman());

  el("#rail-list").addEventListener("click", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".rail-row",
    );
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
    const editBtn = (e.target as HTMLElement).closest<HTMLButtonElement>(
      ".msg-edit-btn",
    );
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
        <div class="connection-bar">
          <div class="connection-bar-left">
            <button type="button" id="connect-btn" data-connected="false">Connect</button>
            <span id="connection-status" class="status-idle">not connected</span>
          </div>
          <div class="connection-bar-center">
            <button type="button" id="provider-config-btn" class="btn-ghost btn-icon btn-provider-alien topbar-toggle" title="Provider and model settings" aria-label="Provider settings">
              ${ICON_SPACE_INVADER}
            </button>
          </div>
          <div class="connection-bar-right">
            <span id="token-budget" class="muted"></span>
            <button type="button" id="toggle-sessions-btn" class="btn-ghost topbar-toggle" aria-pressed="true" title="Hide sessions panel">Hide chats</button>
            <button type="button" id="toggle-map-btn" class="btn-ghost topbar-toggle" aria-pressed="true" title="Hide conversation map">Hide map</button>
          </div>
        </div>
      </header>
      <div class="chat-workspace">
        <aside class="sessions-sidebar" aria-label="Chat sessions">
          <div class="sessions-sidebar-header">
            <span class="sessions-sidebar-title">Chats</span>
            <button type="button" id="session-new" class="btn-secondary">New</button>
          </div>
          <div id="session-list" class="session-list" role="listbox" aria-label="Sessions"></div>
        </aside>
        <div class="chat-main">
          <div class="chat-split">
            <main id="transcript" class="transcript" aria-label="Active conversation"></main>
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
          </div>
          <footer class="input-bar">
            <textarea id="message-input" placeholder="Type a message… (Shift+click message to bookmark)" rows="3"></textarea>
            <button type="button" id="send-btn">Send</button>
          </footer>
          <div id="session-edit-overlay" class="session-edit-overlay" hidden tabindex="-1">
            <div class="session-edit-dialog" role="dialog" aria-modal="true" aria-labelledby="session-edit-title-heading">
              <header class="session-edit-header">
                <h2 id="session-edit-title-heading">Chat settings</h2>
                <button type="button" id="session-edit-cancel" class="btn-secondary">Close</button>
              </header>
              <div class="session-edit-scroll">
                <section class="session-edit-section" aria-label="Chat details">
                  <h3 class="session-edit-section-title">Chat</h3>
                  <label class="session-edit-field">
                    <span class="session-edit-label">Name</span>
                    <input id="session-edit-title" type="text" maxlength="120" />
                  </label>
                  <label class="session-edit-field">
                    <span class="session-edit-label">Description</span>
                    <textarea id="session-edit-summary" rows="2" maxlength="500" placeholder="Optional note about this chat"></textarea>
                  </label>
                </section>
                <section class="session-edit-section" aria-label="Agent context">
                  <h3 class="session-edit-section-title">Agent context</h3>
                  <p id="session-context-summary" class="session-context-summary muted">Loading…</p>
                  <div class="session-context-body">
                    <div class="context-section">
                      <h4 class="context-section-title">Personas</h4>
                      <ul id="context-agents-list" class="context-list"></ul>
                    </div>
                    <div class="context-section">
                      <h4 class="context-section-title">Rules</h4>
                      <ul id="context-rules-list" class="context-list"></ul>
                    </div>
                    <div class="context-section">
                      <h4 class="context-section-title">Skills <span class="muted">index in prompt · full via load_skill</span></h4>
                      <ul id="context-skills-list" class="context-list"></ul>
                    </div>
                  </div>
                </section>
              </div>
              <footer class="session-edit-footer">
                <button type="button" id="session-edit-save" class="btn-secondary">Save chat details</button>
              </footer>
            </div>
          </div>
          <div id="session-delete-overlay" class="session-delete-overlay" hidden tabindex="-1">
            <div class="session-delete-dialog" role="alertdialog" aria-modal="true" aria-labelledby="session-delete-heading" aria-describedby="session-delete-warning">
              <header class="session-delete-header">
                <div class="session-delete-icon" aria-hidden="true">${ICON_TRASH}</div>
                <div>
                  <p class="session-delete-kicker">Nulqor</p>
                  <h2 id="session-delete-heading">Delete chat</h2>
                </div>
              </header>
              <div class="session-delete-body">
                <p id="session-delete-message" class="session-delete-message"></p>
                <p id="session-delete-warning" class="session-delete-warning muted">This removes the transcript and map markers. It cannot be undone.</p>
                <p id="session-delete-error" class="session-delete-error" hidden></p>
              </div>
              <footer class="session-delete-footer">
                <button type="button" id="session-delete-cancel" class="btn-secondary">Cancel</button>
                <button type="button" id="session-delete-confirm" class="btn-danger">Delete</button>
              </footer>
            </div>
          </div>
        </div>
      </div>
      <div id="provider-config-overlay" class="provider-config-overlay" hidden tabindex="-1">
        <div class="provider-config-dialog" role="dialog" aria-modal="true" aria-labelledby="provider-config-title">
          <header class="provider-config-header">
            <h2 id="provider-config-title">Provider settings</h2>
            <button type="button" id="provider-config-close" class="btn-secondary">Close</button>
          </header>
          <div class="provider-config-body">
            <label class="provider-config-field">
              <span class="provider-config-label">Provider</span>
              <select id="provider-select" title="Local inference backend"></select>
            </label>
            <label class="provider-config-field">
              <span class="provider-config-label">Server URL</span>
              <input id="provider-url" type="text" placeholder="http://localhost:1234" value="http://localhost:1234" />
            </label>
            <p id="provider-vram-hint" class="muted provider-hint" title="8 GB VRAM model tips"></p>
            <div class="provider-config-actions">
              <button type="button" id="fetch-models-btn" class="btn-secondary">Fetch models</button>
              <select id="model-select" disabled title="Active model for Nulqor">
                <option value="">— fetch models —</option>
              </select>
              <button type="button" id="load-eject-btn" class="btn-secondary" disabled>Load</button>
            </div>
            <section class="provider-loaded-section" aria-label="Models loaded in VRAM">
              <div class="provider-config-label">Loaded in VRAM</div>
              <ul id="loaded-models-list" class="loaded-models-list">
                <li class="loaded-models-empty muted">No models loaded in VRAM</li>
              </ul>
            </section>
          </div>
        </div>
      </div>
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
  pinTranscriptScroll = true;
  transcriptHash = "";
  messages = [];
  buildShell(container);
  wirePanelEvents();
  wireTranscriptScroll();
  applyMapVisibility(loadMapVisible());
  applySessionsVisibility(loadSessionsVisible());

  void (async () => {
    applyProviderUi(false, null, false);
    populateModelSelect([], null);
    updateModelActionButtons();
    await loadProviderInfo();
    await loadSessions();
    await refreshAll(true);
    await updateTokenBudget();
    scrollTranscriptToBottom("auto");
    pollTimer = setInterval(() => {
      // Skip work while the window/panel is hidden; resume on visibility.
      if (document.visibilityState === "visible") void refreshAll();
    }, 2000);
  })();
}
