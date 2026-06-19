import { invoke } from "@tauri-apps/api/core";
import "./style.css";

type TabId = "extensions" | "commands" | "skills" | "rules" | "agents";

interface ExtensionRow {
  id: string;
  version: string;
  kind: string;
  requires: string[];
  optional: string[];
  command_count: number;
  fs_scopes: string[];
  manifest_path: string;
  in_profile: boolean;
  enabled: boolean;
  protected: boolean;
}

interface CommandRow {
  id: string;
  owner: string;
  permission: string;
  input_schema: string;
  output_schema: string;
  callable_by: string[];
  enabled?: boolean;
}

interface SkillListRow {
  name: string;
  description: string;
  enabled: boolean;
}

interface RuleListRow {
  filename: string;
  excerpt: string;
  enabled: boolean;
}

interface AgentListRow {
  name: string;
  enabled: boolean;
}

interface SkillForm {
  name: string;
  description: string;
  metadata: string;
  whenToUse: string;
  contract: string;
  steps: string;
  verification: string;
}

const SKILL_SECTIONS = [
  "Metadata",
  "When to use",
  "Contract",
  "Steps",
  "Verification",
] as const;

let rootEl: HTMLElement | null = null;
let activeTab: TabId = "extensions";
let selectedKey = "";
let skillEditMode: "form" | "raw" = "form";
let dirty = false;
let statusText = "Ready";

async function coreInvoke<T>(
  id: string,
  input: Record<string, unknown> = {},
): Promise<T> {
  return invoke<T>("core_invoke", { id, input });
}

function el<T extends HTMLElement>(selector: string): T {
  if (!rootEl) throw new Error("workbench not mounted");
  const node = rootEl.querySelector(selector);
  if (!node) throw new Error(`missing ${selector}`);
  return node as T;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function setStatus(text: string): void {
  statusText = text;
  const node = rootEl?.querySelector(".wb-status");
  if (node) node.textContent = text;
}

function markDirty(value = true): void {
  dirty = value;
  const saveBtn = rootEl?.querySelector<HTMLButtonElement>(".wb-save");
  if (saveBtn) saveBtn.disabled = !dirty;
}

function isRulesIndex(filename: string): boolean {
  const base = filename.replace(/\.[^.]+$/, "");
  return base.toLowerCase() === "index";
}

async function setItemEnabled(
  kind: "extension" | "skill" | "rule" | "agent",
  id: string,
  enabled: boolean,
): Promise<void> {
  setStatus("Updating…");
  try {
    await coreInvoke("workbench:set-enabled@1", { kind, id, enabled });
    setStatus("Updated");
    await refreshList();
  } catch (err) {
    setStatus(`Update failed: ${err}`);
  }
}

function appendToggleListRow(
  listEl: HTMLElement,
  opts: {
    key: string;
    title: string;
    meta: string;
    enabled: boolean;
    selected: boolean;
    kind: "extension" | "skill" | "rule" | "agent";
    toggleDisabled?: boolean;
    toggleTitle?: string;
    onSelect: () => void;
  },
): void {
  const row = document.createElement("div");
  row.className = `wb-list-item${opts.selected ? " is-selected" : ""}${opts.enabled ? "" : " is-disabled"}`;
  row.dataset.key = opts.key;

  const toggleLabel = document.createElement("label");
  toggleLabel.className = "wb-toggle-label";
  toggleLabel.title =
    opts.toggleTitle ??
    (opts.enabled ? "Enabled — included in runtime" : "Disabled — excluded from runtime");

  const toggle = document.createElement("input");
  toggle.type = "checkbox";
  toggle.className = "wb-toggle";
  toggle.checked = opts.enabled;
  toggle.disabled = opts.toggleDisabled ?? false;
  toggle.addEventListener("click", (e) => e.stopPropagation());
  toggle.addEventListener("change", () => {
    void setItemEnabled(opts.kind, opts.key, toggle.checked);
  });
  toggleLabel.appendChild(toggle);

  const body = document.createElement("button");
  body.type = "button";
  body.className = "wb-list-body";
  body.innerHTML = `<span class="wb-list-title">${escapeHtml(opts.title)}</span><span class="wb-list-meta">${escapeHtml(opts.meta)}</span>`;
  body.addEventListener("click", opts.onSelect);

  row.append(toggleLabel, body);
  listEl.append(row);
}

function parseSkillBody(body: string): SkillForm {
  const stripped = body.trimStart().replace(/^---\r?\n/, "");
  const end = stripped.search(/\r?\n---/);
  let frontYaml = "";
  let rest = stripped;
  if (end >= 0) {
    frontYaml = stripped.slice(0, end).trim();
    const after = stripped.slice(end).replace(/^---\r?\n?/, "");
    rest = after.trimStart();
  }

  let name = "";
  let description = "";
  for (const line of frontYaml.split(/\r?\n/)) {
    if (line.startsWith("name:")) name = line.slice(5).trim();
    if (line.startsWith("description:")) description = line.slice(12).trim();
  }

  const sections: Record<string, string> = {};
  const parts = rest.split(/\r?\n(?=## )/);
  for (const part of parts) {
    const trimmed = part.trim();
    if (!trimmed.startsWith("## ")) continue;
    const nl = trimmed.indexOf("\n");
    const heading = trimmed.slice(3, nl >= 0 ? nl : undefined).trim();
    const content = nl >= 0 ? trimmed.slice(nl + 1).trim() : "";
    sections[heading] = content;
  }

  return {
    name,
    description,
    metadata: sections["Metadata"] ?? "",
    whenToUse: sections["When to use"] ?? "",
    contract: sections["Contract"] ?? "",
    steps: sections["Steps"] ?? "",
    verification: sections["Verification"] ?? "",
  };
}

function buildSkillBody(form: SkillForm): string {
  const desc =
    form.description.includes("\n") || form.description.length > 80
      ? `>\n  ${form.description.replace(/\n/g, "\n  ")}`
      : form.description;
  const sections = SKILL_SECTIONS.map((title) => {
    const map: Record<string, string> = {
      Metadata: form.metadata,
      "When to use": form.whenToUse,
      Contract: form.contract,
      Steps: form.steps,
      Verification: form.verification,
    };
    return `## ${title}\n\n${map[title]?.trim() ?? ""}`;
  }).join("\n\n---\n\n");

  return `---\nname: ${form.name}\ndescription: ${desc}\n---\n\n${sections}\n`;
}

function renderShell(): void {
  if (!rootEl) return;
  rootEl.innerHTML = `
    <div class="wb-root">
      <header class="wb-header">
        <h1 class="wb-title">Workbench</h1>
        <nav class="wb-tabs" role="tablist">
          ${(["extensions", "commands", "skills", "rules", "agents"] as TabId[])
            .map(
              (tab) =>
                `<button type="button" class="wb-tab${tab === activeTab ? " is-active" : ""}" data-tab="${tab}">${tab}</button>`,
            )
            .join("")}
        </nav>
      </header>
      <div class="wb-body">
        <aside class="wb-list" aria-label="Item list"></aside>
        <main class="wb-detail" aria-label="Inspector"></main>
      </div>
      <footer class="wb-footer">
        <span class="wb-status">${escapeHtml(statusText)}</span>
        <div class="wb-footer-actions">
          <button type="button" class="wb-save" disabled>Save</button>
        </div>
      </footer>
    </div>
  `;

  rootEl.querySelectorAll<HTMLButtonElement>(".wb-tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      const tab = btn.dataset.tab as TabId;
      if (tab && tab !== activeTab) {
        activeTab = tab;
        selectedKey = "";
        dirty = false;
        renderShell();
        wireFooter();
        void refreshList();
      }
    });
  });

  wireFooter();
}

let footerAbort: AbortController | null = null;

function wireFooter(): void {
  footerAbort?.abort();
  footerAbort = new AbortController();
  const saveBtn = rootEl?.querySelector<HTMLButtonElement>(".wb-save");
  saveBtn?.addEventListener("click", () => void saveCurrent(), {
    signal: footerAbort.signal,
  });
}

async function refreshList(): Promise<void> {
  const listEl = el<HTMLElement>(".wb-list");
  const detailEl = el<HTMLElement>(".wb-detail");
  listEl.innerHTML = `<p class="wb-loading">Loading…</p>`;
  if (!dirty) {
    detailEl.innerHTML = "";
  }

  try {
    if (activeTab === "extensions") {
      const data = await coreInvoke<{ extensions: ExtensionRow[] }>(
        "extensions:list@1",
      );
      renderExtensionList(data.extensions);
    } else if (activeTab === "commands") {
      const data = await coreInvoke<{ commands: CommandRow[] }>(
        "commands:catalog@1",
      );
      renderCommandList(data.commands);
    } else if (activeTab === "skills") {
      const data = await coreInvoke<{ skills: SkillListRow[] }>(
        "context-editor:list-skills@1",
      );
      renderSkillList(data.skills);
    } else if (activeTab === "rules") {
      const data = await coreInvoke<{ rules: RuleListRow[] }>(
        "context-editor:list-rules@1",
      );
      renderRuleList(data.rules.filter((r) => !isRulesIndex(r.filename)));
    } else {
      const data = await coreInvoke<{ agents: AgentListRow[] }>(
        "context-editor:list-agents@1",
      );
      renderAgentList(data.agents);
    }
    setStatus("Ready");
  } catch (err) {
    listEl.innerHTML = `<p class="wb-error">${escapeHtml(String(err))}</p>`;
    setStatus(`Error: ${err}`);
  }
}

function renderExtensionList(rows: ExtensionRow[]): void {
  const listEl = el<HTMLElement>(".wb-list");
  listEl.innerHTML = "";
  for (const row of rows) {
    appendToggleListRow(listEl, {
      key: row.id,
      title: row.id,
      meta: `${row.kind} · v${row.version}${row.in_profile ? "" : " · not loaded"}`,
      enabled: row.enabled,
      selected: selectedKey === row.id,
      kind: "extension",
      toggleDisabled: !row.in_profile || row.protected,
      toggleTitle: row.protected
        ? "Core extension — cannot be disabled"
        : !row.in_profile
          ? "Not in nulqor.toml enabled_extensions — restart required to load"
          : undefined,
      onSelect: () => {
        selectedKey = row.id;
        renderExtensionList(rows);
        void showExtensionDetail(row.id, rows);
      },
    });
  }
  if (selectedKey) void showExtensionDetail(selectedKey, rows);
}

async function showExtensionDetail(
  id: string,
  rows: ExtensionRow[],
): Promise<void> {
  const detailEl = el<HTMLElement>(".wb-detail");
  const row = rows.find((r) => r.id === id);
  if (!row) {
    detailEl.innerHTML = "";
    return;
  }

  let graphHtml = "";
  try {
    const graph = await coreInvoke<{
      nodes: { id: string; kind: string; enabled: boolean }[];
      edges: { from: string; to: string; kind: string }[];
    }>("extensions:graph@1");
    const deps = graph.edges.filter((e) => e.from === id);
    const dependents = graph.edges.filter((e) => e.to === id);
    graphHtml = `
      <section class="wb-section">
        <h3>Dependencies</h3>
        ${
          deps.length
            ? `<ul>${deps.map((e) => `<li><code>${escapeHtml(e.to)}</code> (${escapeHtml(e.kind)})</li>`).join("")}</ul>`
            : '<p class="wb-muted">None</p>'
        }
      </section>
      <section class="wb-section">
        <h3>Dependents</h3>
        ${
          dependents.length
            ? `<ul>${dependents.map((e) => `<li><code>${escapeHtml(e.from)}</code></li>`).join("")}</ul>`
            : '<p class="wb-muted">None</p>'
        }
      </section>`;
  } catch {
    graphHtml = "";
  }

  detailEl.innerHTML = `
    <div class="wb-detail-head">
      <h2>${escapeHtml(row.id)}</h2>
      <p class="wb-muted">${escapeHtml(row.manifest_path)}</p>
    </div>
    <dl class="wb-dl">
      <dt>Kind</dt><dd>${escapeHtml(row.kind)}</dd>
      <dt>Version</dt><dd>${escapeHtml(row.version)}</dd>
      <dt>In startup profile</dt><dd>${row.in_profile ? "yes" : "no"}</dd>
      <dt>Runtime enabled</dt><dd>${row.enabled ? "yes" : "no"}</dd>
      <dt>Declared commands</dt><dd>${row.command_count}</dd>
      <dt>Requires</dt><dd>${row.requires.length ? row.requires.map((r) => `<code>${escapeHtml(r)}</code>`).join(", ") : "—"}</dd>
      <dt>Optional</dt><dd>${row.optional.length ? row.optional.map((r) => `<code>${escapeHtml(r)}</code>`).join(", ") : "—"}</dd>
      <dt>FS scopes</dt><dd>${row.fs_scopes.length ? row.fs_scopes.map((s) => `<code>${escapeHtml(s)}</code>`).join(", ") : "—"}</dd>
    </dl>
    ${graphHtml}
  `;
  markDirty(false);
}

function renderCommandList(rows: CommandRow[]): void {
  const byOwner = new Map<string, CommandRow[]>();
  for (const row of rows) {
    const list = byOwner.get(row.owner) ?? [];
    list.push(row);
    byOwner.set(row.owner, list);
  }
  const owners = [...byOwner.keys()].sort();

  const listEl = el<HTMLElement>(".wb-list");
  listEl.innerHTML = "";
  for (const owner of owners) {
    const cmds = byOwner.get(owner) ?? [];
    const ownerEnabled = cmds[0]?.enabled !== false;
    appendToggleListRow(listEl, {
      key: owner,
      title: owner,
      meta: `${cmds.length} commands${ownerEnabled ? "" : " · extension disabled"}`,
      enabled: ownerEnabled,
      selected: selectedKey === owner,
      kind: "extension",
      toggleDisabled: owner === "host" || owner === "registry",
      toggleTitle:
        owner === "host" || owner === "registry"
          ? "Core extension — cannot be disabled"
          : undefined,
      onSelect: () => {
        selectedKey = owner;
        renderCommandList(rows);
        showCommandDetail(owner, byOwner);
      },
    });
  }

  if (selectedKey && byOwner.has(selectedKey)) {
    showCommandDetail(selectedKey, byOwner);
  }
}

function showCommandDetail(
  owner: string,
  byOwner: Map<string, CommandRow[]>,
): void {
  const detailEl = el<HTMLElement>(".wb-detail");
  const cmds = byOwner.get(owner) ?? [];
  const ownerDisabled = cmds.some((c) => c.enabled === false);
  detailEl.innerHTML = `
    <div class="wb-detail-head${ownerDisabled ? " is-disabled" : ""}"><h2>${escapeHtml(owner)}</h2></div>
    <div class="wb-command-table">
      ${cmds
        .map(
          (cmd) => `
        <article class="wb-command-card${cmd.enabled === false ? " is-disabled" : ""}">
          <h3><code>${escapeHtml(cmd.id)}</code>${cmd.enabled === false ? ' <span class="wb-muted">(disabled)</span>' : ""}</h3>
          <dl class="wb-dl compact">
            <dt>Permission</dt><dd>${escapeHtml(cmd.permission)}</dd>
            <dt>Callable by</dt><dd>${cmd.callable_by.map((c) => escapeHtml(c)).join(", ") || "—"}</dd>
            <dt>Input</dt><dd><pre>${escapeHtml(cmd.input_schema || "{}")}</pre></dd>
            <dt>Output</dt><dd><pre>${escapeHtml(cmd.output_schema || "{}")}</pre></dd>
          </dl>
        </article>`,
        )
        .join("")}
    </div>
  `;
  markDirty(false);
}

function renderSkillList(rows: SkillListRow[]): void {
  const listEl = el<HTMLElement>(".wb-list");
  listEl.innerHTML = "";
  for (const row of rows) {
    appendToggleListRow(listEl, {
      key: row.name,
      title: row.name,
      meta: row.description,
      enabled: row.enabled,
      selected: selectedKey === row.name,
      kind: "skill",
      onSelect: () => {
        selectedKey = row.name;
        renderSkillList(rows);
        void showSkillEditor(row.name);
      },
    });
  }
  if (selectedKey) void showSkillEditor(selectedKey);
}

function renderRuleList(rows: RuleListRow[]): void {
  const listEl = el<HTMLElement>(".wb-list");
  listEl.innerHTML = "";
  for (const row of rows) {
    appendToggleListRow(listEl, {
      key: row.filename,
      title: row.filename,
      meta: row.excerpt,
      enabled: row.enabled,
      selected: selectedKey === row.filename,
      kind: "rule",
      onSelect: () => {
        selectedKey = row.filename;
        renderRuleList(rows);
        void showRuleEditor(row.filename);
      },
    });
  }
  if (selectedKey) void showRuleEditor(selectedKey);
}

function renderAgentList(rows: AgentListRow[]): void {
  const listEl = el<HTMLElement>(".wb-list");
  listEl.innerHTML = "";
  for (const row of rows) {
    appendToggleListRow(listEl, {
      key: row.name,
      title: row.name,
      meta: row.name === "default" ? "AGENTS.md" : `agents/${row.name}.md`,
      enabled: row.enabled,
      selected: selectedKey === row.name,
      kind: "agent",
      onSelect: () => {
        selectedKey = row.name;
        renderAgentList(rows);
        void showAgentEditor(row.name);
      },
    });
  }
  if (selectedKey) void showAgentEditor(selectedKey);
}

async function showSkillEditor(name: string): Promise<void> {
  const detailEl = el<HTMLElement>(".wb-detail");
  detailEl.innerHTML = `<p class="wb-loading">Loading skill…</p>`;
  try {
    const skill = await coreInvoke<{
      name: string;
      description: string;
      body: string;
    }>("context-editor:load-skill@1", { name });
    const list = await coreInvoke<{ skills: SkillListRow[] }>(
      "context-editor:list-skills@1",
    );
    const enabled =
      list.skills.find((s) => s.name === name)?.enabled ?? true;
    const form = parseSkillBody(skill.body);
    renderSkillEditor(form, skill.body, enabled);
  } catch (err) {
    detailEl.innerHTML = `<p class="wb-error">${escapeHtml(String(err))}</p>`;
  }
}

function renderSkillEditor(form: SkillForm, rawBody: string, enabled = true): void {
  const detailEl = el<HTMLElement>(".wb-detail");
  detailEl.innerHTML = `
    <div class="wb-detail-head">
      <h2>${escapeHtml(form.name || selectedKey)}</h2>
      <div class="wb-mode-toggle">
        <button type="button" class="wb-mode${skillEditMode === "form" ? " is-active" : ""}" data-mode="form">Form</button>
        <button type="button" class="wb-mode${skillEditMode === "raw" ? " is-active" : ""}" data-mode="raw">Raw</button>
      </div>
    </div>
    <div class="wb-editor"></div>
  `;

  const editorEl = el<HTMLElement>(".wb-editor");
  if (skillEditMode === "raw") {
    editorEl.innerHTML = `<textarea class="wb-raw" spellcheck="false"${enabled ? "" : " readonly"}>${escapeHtml(rawBody)}</textarea>`;
    editorEl
      .querySelector("textarea")
      ?.addEventListener("input", () => markDirty(true));
  } else {
    editorEl.innerHTML = `
      <label class="wb-field"><span>Name</span><input class="wb-input wb-skill-name" value="${escapeHtml(form.name)}"${enabled ? "" : " readonly"} /></label>
      <label class="wb-field"><span>Description</span><textarea class="wb-input wb-skill-desc" rows="2"${enabled ? "" : " readonly"}>${escapeHtml(form.description)}</textarea></label>
      <label class="wb-field"><span>Metadata</span><textarea class="wb-input wb-skill-metadata" rows="4"${enabled ? "" : " readonly"}>${escapeHtml(form.metadata)}</textarea></label>
      <label class="wb-field"><span>When to use</span><textarea class="wb-input wb-skill-when" rows="4"${enabled ? "" : " readonly"}>${escapeHtml(form.whenToUse)}</textarea></label>
      <label class="wb-field"><span>Contract</span><textarea class="wb-input wb-skill-contract" rows="5"${enabled ? "" : " readonly"}>${escapeHtml(form.contract)}</textarea></label>
      <label class="wb-field"><span>Steps</span><textarea class="wb-input wb-skill-steps" rows="6"${enabled ? "" : " readonly"}>${escapeHtml(form.steps)}</textarea></label>
      <label class="wb-field"><span>Verification</span><textarea class="wb-input wb-skill-verification" rows="4"${enabled ? "" : " readonly"}>${escapeHtml(form.verification)}</textarea></label>
    `;
    editorEl.querySelectorAll("input, textarea").forEach((node) => {
      node.addEventListener("input", () => markDirty(true));
    });
  }

  detailEl.querySelectorAll<HTMLButtonElement>(".wb-mode").forEach((btn) => {
    btn.addEventListener("click", () => {
      void (async () => {
        const mode = btn.dataset.mode as "form" | "raw";
        if (mode === skillEditMode) return;
        skillEditMode = mode;
        await new Promise<void>((r) => requestAnimationFrame(() => r()));
        if (mode === "raw") {
          const currentForm = readSkillForm();
          renderSkillEditor(currentForm, buildSkillBody(currentForm), enabled);
        } else {
          const raw = el<HTMLTextAreaElement>(".wb-raw").value;
          renderSkillEditor(parseSkillBody(raw), raw, enabled);
        }
      })();
    });
  });

  detailEl.classList.toggle("is-disabled", !enabled);
  markDirty(false);
}

function readSkillForm(): SkillForm {
  if (skillEditMode === "raw") {
    return parseSkillBody(el<HTMLTextAreaElement>(".wb-raw").value);
  }
  return {
    name: el<HTMLInputElement>(".wb-skill-name").value.trim(),
    description: el<HTMLTextAreaElement>(".wb-skill-desc").value.trim(),
    metadata: el<HTMLTextAreaElement>(".wb-skill-metadata").value,
    whenToUse: el<HTMLTextAreaElement>(".wb-skill-when").value,
    contract: el<HTMLTextAreaElement>(".wb-skill-contract").value,
    steps: el<HTMLTextAreaElement>(".wb-skill-steps").value,
    verification: el<HTMLTextAreaElement>(".wb-skill-verification").value,
  };
}

async function showRuleEditor(filename: string): Promise<void> {
  const detailEl = el<HTMLElement>(".wb-detail");
  detailEl.innerHTML = `<p class="wb-loading">Loading rule…</p>`;
  try {
    const rule = await coreInvoke<{ filename: string; body: string }>(
      "context-editor:load-rule@1",
      { filename },
    );
    const list = await coreInvoke<{ rules: RuleListRow[] }>(
      "context-editor:list-rules@1",
    );
    const enabled =
      list.rules.find((r) => r.filename === filename)?.enabled ?? true;
    detailEl.innerHTML = `
      <div class="wb-detail-head"><h2>${escapeHtml(rule.filename)}</h2></div>
      <textarea class="wb-raw wb-rule-body" spellcheck="false"${enabled ? "" : " readonly"}>${escapeHtml(rule.body)}</textarea>
    `;
    detailEl.classList.toggle("is-disabled", !enabled);
    el<HTMLTextAreaElement>(".wb-rule-body").addEventListener("input", () =>
      markDirty(true),
    );
    markDirty(false);
  } catch (err) {
    detailEl.innerHTML = `<p class="wb-error">${escapeHtml(String(err))}</p>`;
  }
}

async function showAgentEditor(name: string): Promise<void> {
  const detailEl = el<HTMLElement>(".wb-detail");
  detailEl.innerHTML = `<p class="wb-loading">Loading agent…</p>`;
  try {
    const agent = await coreInvoke<{
      name: string;
      path: string;
      body: string;
    }>("context-editor:load-agent@1", { name });
    const list = await coreInvoke<{ agents: AgentListRow[] }>(
      "context-editor:list-agents@1",
    );
    const enabled = list.agents.find((a) => a.name === name)?.enabled ?? true;
    detailEl.innerHTML = `
      <div class="wb-detail-head">
        <h2>${escapeHtml(agent.name)}</h2>
        <p class="wb-muted">${escapeHtml(agent.path)}</p>
      </div>
      <textarea class="wb-raw wb-agent-body" spellcheck="false"${enabled ? "" : " readonly"}>${escapeHtml(agent.body)}</textarea>
    `;
    detailEl.classList.toggle("is-disabled", !enabled);
    el<HTMLTextAreaElement>(".wb-agent-body").addEventListener("input", () =>
      markDirty(true),
    );
    markDirty(false);
  } catch (err) {
    detailEl.innerHTML = `<p class="wb-error">${escapeHtml(String(err))}</p>`;
  }
}

async function saveCurrent(): Promise<void> {
  if (!dirty || !selectedKey) return;
  if (rootEl?.querySelector(".wb-detail.is-disabled")) {
    setStatus("Enable this item before saving");
    return;
  }
  const keyAtStart = selectedKey;
  setStatus("Saving…");
  try {
    if (activeTab === "skills") {
      const body =
        skillEditMode === "raw"
          ? el<HTMLTextAreaElement>(".wb-raw").value
          : buildSkillBody(readSkillForm());
      const name = readSkillForm().name || keyAtStart;
      await coreInvoke("context-editor:save-skill@1", { name, body });
      if (selectedKey === keyAtStart) {
        selectedKey = name;
      }
    } else if (activeTab === "rules") {
      const body = el<HTMLTextAreaElement>(".wb-rule-body").value;
      await coreInvoke("context-editor:save-rule@1", {
        filename: keyAtStart,
        body,
      });
    } else if (activeTab === "agents") {
      const body = el<HTMLTextAreaElement>(".wb-agent-body").value;
      await coreInvoke("context-editor:save-agent@1", {
        name: keyAtStart,
        body,
      });
    } else {
      return;
    }
    await coreInvoke("context-editor:reload@1");
    markDirty(false);
    setStatus("Saved");
    await refreshList();
  } catch (err) {
    setStatus(`Save failed: ${err}`);
  }
}

export function mount(container: HTMLElement): void {
  rootEl = container;
  rootEl.classList.add("workbench-body");
  activeTab = "extensions";
  selectedKey = "";
  skillEditMode = "form";
  dirty = false;
  renderShell();
  void refreshList();
}
