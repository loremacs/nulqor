// State
let isConnected = false;
let currentModel = "";
let currentAgent = "default";
let humanParticipantName = "You";
let selectedFile = null;
let renderedMessageIds = new Set();
let pendingHumanTurn = false;
let ideStreamElement = null;
let ideStreamReasoningElement = null;
let ideStreamMessageDiv = null;
let ideStreamParticipant = null;

function participantLabel(metadata) {
    if (metadata?.participant_name) {
        return metadata.participant_name;
    }
    const driver = metadata?.driver || "human";
    if (driver === "human") return humanParticipantName || "You";
    if (driver === "ide") return "External agent";
    return driver;
}

function isHumanParticipant(metadata) {
    return (metadata?.driver || "human") === "human";
}

function userHeaderText(metadata) {
    return participantLabel(metadata);
}

function assistantHeaderText(metadata, extras = {}) {
    const model = metadata?.model || extras.model || currentModel;
    const latency = metadata?.latency_ms ?? extras.latency ?? 0;
    const tokens = metadata?.token_count ?? extras.tokens ?? 0;
    const asker = participantLabel(metadata);
    const parts = [`Model • ${model || "unknown"}`];
    if (asker) {
        parts.push(`reply to ${asker}`);
    }
    if (latency) parts.push(`${latency}ms`);
    if (tokens) parts.push(`${tokens} tokens`);
    return parts.join(" · ");
}

function messageHeaderText(msg) {
    if (msg.role === "assistant") {
        return assistantHeaderText(msg.metadata);
    }
    return userHeaderText(msg.metadata);
}

function ensureReasoningBlock(messageDiv, { open = false } = {}) {
    let details = messageDiv.querySelector(".reasoning-details");
    if (!details) {
        details = document.createElement("details");
        details.className = "reasoning-details";
        const summary = document.createElement("summary");
        summary.textContent = "Thinking";
        const pre = document.createElement("pre");
        pre.className = "reasoning-content";
        details.appendChild(summary);
        details.appendChild(pre);
        const content = messageDiv.querySelector(".message-content");
        if (content) {
            messageDiv.insertBefore(details, content);
        } else {
            messageDiv.appendChild(details);
        }
    }
    details.open = open;
    return details.querySelector(".reasoning-content");
}

function attachReasoningFromMetadata(messageDiv, metadata) {
    const text = metadata?.reasoning_content;
    if (!text || !String(text).trim()) {
        return;
    }
    ensureReasoningBlock(messageDiv).textContent = text;
}

// DOM Elements
const endpointInput = document.getElementById("endpoint");
const connectBtn = document.getElementById("connect-btn");
const statusDot = document.getElementById("status-dot");
const statusLabel = document.getElementById("status-label");
const settingsConnectionLabel = document.getElementById("settings-connection-label");
const headerModel = document.getElementById("header-model");
const humanParticipantInput = document.getElementById("human-participant-name");
const generateParticipantBtn = document.getElementById("generate-participant-btn");
const saveParticipantBtn = document.getElementById("save-participant-btn");
const participantNameHint = document.getElementById("participant-name-hint");
const headerAgent = document.getElementById("header-agent");
const modelSelect = document.getElementById("model-select");
const agentSelect = document.getElementById("agent-select");
const tokenBadge = document.getElementById("token-badge");
const latencyBadge = document.getElementById("latency-badge");
const settingsBtn = document.getElementById("settings-btn");
const settingsOverlay = document.getElementById("settings-overlay");
const settingsBackdrop = document.getElementById("settings-backdrop");
const settingsClose = document.getElementById("settings-close");
const reloadContextBtn = document.getElementById("reload-context-btn");
const togglePanelBtn = document.getElementById("toggle-panel");
const leftPanel = document.getElementById("left-panel");
const userInput = document.getElementById("user-input");
const sendBtn = document.getElementById("send-btn");
const chatTranscript = document.getElementById("chat-transcript");
const driverIndicator = document.getElementById("driver-indicator");
const fileEditor = document.getElementById("file-editor");
const saveFileBtn = document.getElementById("save-file");

// Initialize
document.addEventListener("DOMContentLoaded", () => {
    if (!backendReady()) {
        showBackendError();
        return;
    }
    loadContext();
    loadSettings();
    loadHumanParticipantName();
    if (togglePanelBtn) {
        togglePanelBtn.textContent = "◀";
        togglePanelBtn.title = "Hide context editor";
    }
    setupEventListeners();
    setupStreamEvents();
    setupTranscriptEvents();
    autoConnect().then(() => loadTranscript());
});

function driverLabel(driver) {
    if (driver === "human") return humanParticipantName || "You";
    if (driver === "ide") return "External agent";
    return driver || "Unknown";
}

function updateParticipantUI(name) {
    humanParticipantName = name || humanParticipantName || "You";
    if (humanParticipantInput) {
        humanParticipantInput.value = humanParticipantName;
    }
}

async function loadHumanParticipantName() {
    if (!window.go?.main?.App?.GetHumanParticipantName) return;
    try {
        const name = await window.go.main.App.GetHumanParticipantName();
        updateParticipantUI(name);
    } catch (error) {
        console.error("Failed to load participant name:", error);
    }
}

async function saveHumanParticipantName(name) {
    if (!window.go?.main?.App?.SetHumanParticipantName) return;
    try {
        const saved = await window.go.main.App.SetHumanParticipantName(name || "");
        updateParticipantUI(saved);
        if (participantNameHint) {
            participantNameHint.textContent = `Chat will show "${saved}" on your messages.`;
        }
    } catch (error) {
        console.error("Failed to save participant name:", error);
        alert("Failed to save name: " + formatError(error));
    }
}

async function generateHumanParticipantName() {
    if (!window.go?.main?.App?.GenerateParticipantName) return;
    try {
        const suggestion = await window.go.main.App.GenerateParticipantName();
        if (humanParticipantInput) {
            humanParticipantInput.value = suggestion;
        }
        await saveHumanParticipantName(suggestion);
    } catch (error) {
        console.error("Failed to generate participant name:", error);
    }
}

function setupTranscriptEvents() {
    if (!window.runtime?.EventsOn) {
        return;
    }

    window.runtime.EventsOn("transcript-event", (event) => {
        if (!event?.type) return;

        switch (event.type) {
            case "message_added":
                handleTranscriptMessage(event.message);
                break;
            case "stream_start":
                if (pendingHumanTurn) return;
                startIdeStream(event.stream_id, event.session?.model, event.participant);
                break;
            case "stream_delta":
                if (ideStreamElement && event.delta) {
                    ideStreamElement.textContent += event.delta;
                    chatTranscript.scrollTop = chatTranscript.scrollHeight;
                }
                break;
            case "reasoning_delta":
                if (ideStreamMessageDiv && event.delta) {
                    if (!ideStreamReasoningElement) {
                        ideStreamReasoningElement = ensureReasoningBlock(ideStreamMessageDiv, { open: true });
                    }
                    ideStreamReasoningElement.textContent += event.delta;
                    const details = ideStreamReasoningElement.closest(".reasoning-details");
                    if (details) {
                        details.open = true;
                    }
                    chatTranscript.scrollTop = chatTranscript.scrollHeight;
                }
                break;
            case "stream_done":
                finishIdeStream(event.message);
                break;
        }
    });
}

function handleTranscriptMessage(msg) {
    if (!msg?.id || renderedMessageIds.has(msg.id)) {
        return;
    }

    const driver = msg.metadata?.driver || "human";
    renderedMessageIds.add(msg.id);

    // Human turns are rendered locally by sendMessage(); only track ids here.
    if (driver === "human") {
        return;
    }

    if (msg.role === "assistant" && ideStreamMessageDiv) {
        finishIdeStream(msg);
        driverIndicator.textContent = `Driver: ${userHeaderText(msg.metadata)}`;
        return;
    }

    renderBackendMessage(msg);
    driverIndicator.textContent = `Driver: ${driverLabel(driver)}`;
}

function renderBackendMessage(msg) {
    const driver = msg.metadata?.driver || "human";
    const humanTurn = isHumanParticipant(msg.metadata);

    if (msg.role === "assistant" && ideStreamMessageDiv) {
        finishIdeStream(msg);
        return;
    }

    const welcome = chatTranscript.querySelector(".welcome-message");
    if (welcome) welcome.remove();

    const messageDiv = document.createElement("div");
    messageDiv.className = `message ${msg.role}`;
    if (!humanTurn) {
        messageDiv.classList.add("external-agent");
    }
    messageDiv.dataset.messageId = msg.id;
    messageDiv.dataset.participant = participantLabel(msg.metadata);

    const header = document.createElement("div");
    header.className = "message-header";
    header.textContent = messageHeaderText(msg);

    const contentDiv = document.createElement("div");
    contentDiv.className = "message-content";
    contentDiv.textContent = msg.content;

    messageDiv.appendChild(header);
    messageDiv.appendChild(contentDiv);
    attachReasoningFromMetadata(messageDiv, msg.metadata);
    chatTranscript.appendChild(messageDiv);
    chatTranscript.scrollTop = chatTranscript.scrollHeight;

    const tokens = msg.metadata?.token_count || 0;
    const latency = msg.metadata?.latency_ms || 0;
    if (tokens) tokenBadge.textContent = `Tokens: ${tokens}`;
    if (latency) latencyBadge.textContent = `Latency: ${latency}ms`;
}

function startIdeStream(streamId, model, participant) {
    const welcome = chatTranscript.querySelector(".welcome-message");
    if (welcome) welcome.remove();

    ideStreamParticipant = participant || "External agent";
    ideStreamElement = document.createElement("div");
    ideStreamElement.className = "message-content";
    ideStreamMessageDiv = addMessageShell("assistant", ideStreamParticipant, ideStreamElement, true);
    const header = ideStreamMessageDiv.querySelector(".message-header");
    header.textContent = assistantHeaderText(
        { driver: ideStreamParticipant, participant_name: ideStreamParticipant, model },
        { model }
    );
    ideStreamMessageDiv.dataset.streamId = streamId || "";
}

function finishIdeStream(msg) {
    if (!ideStreamMessageDiv) {
        if (msg?.id && !renderedMessageIds.has(msg.id)) {
            renderedMessageIds.add(msg.id);
            renderBackendMessage(msg);
        }
        return;
    }

    if (msg) {
        if (!renderedMessageIds.has(msg.id)) {
            renderedMessageIds.add(msg.id);
        }
        if (ideStreamElement) {
            ideStreamElement.textContent = msg.content;
        }
        attachReasoningFromMetadata(ideStreamMessageDiv, msg.metadata);
        const reasoningDetails = ideStreamMessageDiv.querySelector(".reasoning-details");
        if (reasoningDetails) {
            reasoningDetails.open = false;
        }
        const header = ideStreamMessageDiv.querySelector(".message-header");
        header.textContent = assistantHeaderText(msg.metadata);
        ideStreamMessageDiv.dataset.messageId = msg.id;
        ideStreamMessageDiv.dataset.participant = participantLabel(msg.metadata);
        const tokens = msg.metadata?.token_count || 0;
        const latency = msg.metadata?.latency_ms || 0;
        if (tokens) tokenBadge.textContent = `Tokens: ${tokens}`;
        if (latency) latencyBadge.textContent = `Latency: ${latency}ms`;
    }

    ideStreamElement = null;
    ideStreamReasoningElement = null;
    ideStreamMessageDiv = null;
    ideStreamParticipant = null;
    if (msg) {
        driverIndicator.textContent = `Driver: ${userHeaderText(msg.metadata)}`;
    } else {
        driverIndicator.textContent = "Driver: External agent";
    }
}

async function loadTranscript() {
    if (!window.go?.main?.App?.GetTranscript) return;

    try {
        const session = await window.go.main.App.GetTranscript();
        if (!session?.messages?.length) return;

        const welcome = chatTranscript.querySelector(".welcome-message");
        if (welcome) welcome.remove();

        session.messages.forEach((msg) => {
            if (msg.id && !renderedMessageIds.has(msg.id)) {
                renderedMessageIds.add(msg.id);
                renderBackendMessage(msg);
            }
        });

        if (session.model) {
            currentModel = session.model;
            if (modelSelect) modelSelect.value = session.model;
            updateHeaderChips();
        }
    } catch (error) {
        console.error("Failed to load transcript:", error);
    }
}

function backendReady() {
    return Boolean(window.go?.main?.App?.TestConnection);
}

function showBackendError() {
    connectBtn.disabled = true;
    connectBtn.textContent = "No backend";
    updateConnectionUI(false, null, "Start the app with wails dev");
}

function formatError(error) {
    if (!error) return "Unknown error";
    if (typeof error === "string") return error;
    return error.message || String(error);
}

function setupStreamEvents() {
    if (!window.runtime?.EventsOn) {
        return;
    }

    let activeStreamMessage = null;
    let activeStreamReasoning = null;

    window.runtime.EventsOn("chat-stream", (payload) => {
        if (!activeStreamMessage) {
            return;
        }
        activeStreamMessage.textContent += payload.content || "";
        chatTranscript.scrollTop = chatTranscript.scrollHeight;
    });

    window.runtime.EventsOn("chat-reasoning-stream", (payload) => {
        if (!activeStreamReasoning) {
            return;
        }
        activeStreamReasoning.textContent += payload.content || "";
        const details = activeStreamReasoning.closest(".reasoning-details");
        if (details) {
            details.open = true;
        }
        chatTranscript.scrollTop = chatTranscript.scrollHeight;
    });

    window.runtime.EventsOn("chat-stream-done", () => {
        activeStreamMessage = null;
        activeStreamReasoning = null;
    });

    window._setActiveStreamMessage = (element) => {
        activeStreamMessage = element;
    };
    window._setActiveStreamReasoning = (element) => {
        activeStreamReasoning = element;
    };
}

function setupEventListeners() {
    connectBtn.addEventListener("click", () => handleConnect());
    settingsBtn.addEventListener("click", openSettings);
    settingsClose.addEventListener("click", closeSettings);
    settingsBackdrop.addEventListener("click", closeSettings);
    reloadContextBtn.addEventListener("click", reloadContext);
    generateParticipantBtn.addEventListener("click", generateHumanParticipantName);
    saveParticipantBtn.addEventListener("click", () => saveHumanParticipantName(humanParticipantInput.value));
    togglePanelBtn.addEventListener("click", togglePanel);
    sendBtn.addEventListener("click", sendMessage);
    saveFileBtn.addEventListener("click", saveFile);

    document.addEventListener("keydown", (e) => {
        if (e.key === "Escape" && !settingsOverlay.classList.contains("hidden")) {
            closeSettings();
        }
    });
    
    userInput.addEventListener("keydown", (e) => {
        if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            sendMessage();
        }
    });

    agentSelect.addEventListener("change", (e) => {
        currentAgent = e.target.value;
        updateHeaderChips();
        updateSystemPromptBadge();
    });

    modelSelect.addEventListener("change", onModelChange);
}

function openSettings() {
    settingsOverlay.classList.remove("hidden");
    settingsOverlay.setAttribute("aria-hidden", "false");
}

function closeSettings() {
    settingsOverlay.classList.add("hidden");
    settingsOverlay.setAttribute("aria-hidden", "true");
}

async function loadSettings() {
    if (!window.go?.main?.App?.GetSettings) return;

    try {
        const settings = await window.go.main.App.GetSettings();
        if (settings.lmstudio?.base_url && endpointInput) {
            endpointInput.value = settings.lmstudio.base_url;
        }
        if (settings.generation) {
            document.getElementById("setting-temperature").value = settings.generation.temperature;
            document.getElementById("setting-max-tokens").value = settings.generation.max_tokens;
            document.getElementById("setting-top-p").value = settings.generation.top_p;
            document.getElementById("setting-top-k").value = settings.generation.top_k;
        }
        if (settings.server) {
            document.getElementById("setting-api-url").value =
                `http://${settings.server.host}:${settings.server.port}`;
        }
        if (settings.paths) {
            document.getElementById("setting-skills-dir").value = settings.paths.skills_dir || "";
            document.getElementById("setting-agents-dir").value = settings.paths.agents_dir || "";
            document.getElementById("setting-rules-dir").value = settings.paths.rules_dir || "";
        }
        if (settings.defaults?.agent) {
            currentAgent = settings.defaults.agent;
            updateHeaderChips();
        }
        if (settings.human_participant_name) {
            updateParticipantUI(settings.human_participant_name);
        }
    } catch (error) {
        console.error("Failed to load settings:", error);
    }
}

async function reloadContext() {
    if (!window.go?.main?.App?.ReloadContext) return;

    reloadContextBtn.disabled = true;
    reloadContextBtn.textContent = "Reloading...";
    try {
        await window.go.main.App.ReloadContext();
        await loadContext();
        updateSystemPromptBadge();
        settingsConnectionLabel.textContent = "Context reloaded from disk";
    } catch (error) {
        console.error("Failed to reload context:", error);
        alert("Failed to reload context: " + formatError(error));
    } finally {
        reloadContextBtn.disabled = false;
        reloadContextBtn.textContent = "Reload skills & rules";
    }
}

function updateConnectionUI(connected, endpoint, errorMessage) {
    statusDot.classList.toggle("connected", connected);
    statusDot.classList.toggle("disconnected", !connected);

    if (connected) {
        const label = endpoint || endpointInput.value;
        statusDot.title = `Connected to ${label}`;
        statusLabel.textContent = "Connected";
        settingsConnectionLabel.textContent = `Connected to ${label}`;
        connectBtn.textContent = "Connected";
    } else {
        statusDot.title = errorMessage || "Not connected";
        statusLabel.textContent = "Disconnected";
        settingsConnectionLabel.textContent = errorMessage || "Not connected";
        connectBtn.textContent = "Connect";
    }
}

function updateHeaderChips() {
    if (headerModel) {
        headerModel.textContent = currentModel || "No model";
    }
    if (headerAgent) {
        headerAgent.textContent = currentAgent || "default";
    }
}

function onModelChange(e) {
    currentModel = e.target.value;
    updateHeaderChips();
}

async function autoConnect() {
    if (endpointInput.value.trim()) {
        await handleConnect({ quiet: true });
    }
}

async function handleConnect(options = {}) {
    const quiet = options.quiet === true;
    const endpoint = endpointInput.value.trim();
    if (!endpoint) {
        if (!quiet) {
            alert("Enter an LM Studio endpoint, e.g. http://localhost:1234/v1");
        }
        return;
    }

    connectBtn.disabled = true;
    connectBtn.textContent = "Connecting...";

    try {
        const result = await window.go.main.App.TestConnection(endpoint);
        const models = result.models || [];
        endpointInput.value = result.endpoint || endpoint;
        updateModelSelect(models);
        updateConnectionUI(true, result.endpoint || endpoint);
        isConnected = true;
    } catch (error) {
        console.error("Connection failed:", error);
        updateConnectionUI(false, null, formatError(error));
        isConnected = false;
        if (!quiet) {
            alert("Failed to connect: " + formatError(error));
        }
    } finally {
        connectBtn.disabled = false;
    }
}

function updateModelSelect(models) {
    modelSelect.innerHTML = "";
    if (!models || models.length === 0) {
        const option = document.createElement("option");
        option.textContent = "No models loaded";
        modelSelect.appendChild(option);
        currentModel = "";
        updateHeaderChips();
        return;
    }

    models.forEach((model) => {
        const option = document.createElement("option");
        option.value = model;
        option.textContent = model;
        modelSelect.appendChild(option);
    });

    currentModel = models[0];
    modelSelect.value = currentModel;
    updateHeaderChips();
}

async function loadContext() {
    try {
        const skills = await window.go.main.App.ListSkills();
        renderSkillsTree(skills);

        const agents = await window.go.main.App.ListAgents();
        renderAgentsTree(agents);
        updateAgentSelect(agents);
        updateSystemPromptBadge();

        const rules = await window.go.main.App.ListRules();
        renderRulesTree(rules);
    } catch (error) {
        console.error("Failed to load context:", error);
    }
}

function renderSkillsTree(skills) {
    const tree = document.getElementById("skills-tree");
    tree.innerHTML = "";
    skills.forEach((skill) => {
        const item = document.createElement("div");
        item.className = "tree-item";
        item.textContent = skill.name;
        item.addEventListener("click", () => loadFile(skill.path, "skill"));
        tree.appendChild(item);
    });
}

function renderAgentsTree(agents) {
    const tree = document.getElementById("agents-tree");
    tree.innerHTML = "";
    Object.keys(agents).forEach((name) => {
        const item = document.createElement("div");
        item.className = "tree-item";
        item.textContent = name;
        item.addEventListener("click", () => loadFile(agents[name].path, "agent"));
        tree.appendChild(item);
    });
}

function renderRulesTree(rules) {
    const tree = document.getElementById("rules-tree");
    tree.innerHTML = "";
    rules.forEach((rule) => {
        const item = document.createElement("div");
        item.className = "tree-item";
        item.textContent = rule.name;
        item.addEventListener("click", () => loadFile(rule.path, "rule"));
        tree.appendChild(item);
    });
}

function updateAgentSelect(agents) {
    agentSelect.innerHTML = "";
    Object.keys(agents).forEach((name) => {
        const option = document.createElement("option");
        option.value = name;
        option.textContent = name;
        agentSelect.appendChild(option);
    });
    if (currentAgent && agents[currentAgent]) {
        agentSelect.value = currentAgent;
    } else if (Object.keys(agents).length > 0) {
        currentAgent = Object.keys(agents)[0];
        agentSelect.value = currentAgent;
    }
    updateHeaderChips();
}

async function loadFile(path, type) {
    try {
        const content = await window.go.main.App.ReadFile(path);
        fileEditor.value = content;
        selectedFile = { path, type };
    } catch (error) {
        console.error("Failed to load file:", error);
    }
}

async function saveFile() {
    if (!selectedFile) {
        alert("No file selected");
        return;
    }

    try {
        await window.go.main.App.WriteFile(selectedFile.path, fileEditor.value);
        alert("File saved successfully");
        loadContext(); // Reload to reflect changes
    } catch (error) {
        console.error("Failed to save file:", error);
        alert("Failed to save file: " + error);
    }
}

function togglePanel() {
    leftPanel.classList.toggle("collapsed");
    const collapsed = leftPanel.classList.contains("collapsed");
    togglePanelBtn.textContent = collapsed ? "☰" : "◀";
    togglePanelBtn.title = collapsed ? "Show context editor" : "Hide context editor";
}

async function sendMessage() {
    const message = userInput.value.trim();
    if (!message) return;

    if (!isConnected) {
        openSettings();
        alert("Connect to LM Studio in Settings first");
        return;
    }

    pendingHumanTurn = true;
    driverIndicator.textContent = `Driver: ${humanParticipantName}`;
    addMessageToTranscript("user", message, humanParticipantName);
    userInput.value = "";

    const assistantContent = document.createElement("div");
    assistantContent.className = "message-content";
    const messageDiv = addMessageShell("assistant", humanParticipantName, assistantContent);
    const assistantHeader = messageDiv.querySelector(".message-header");
    assistantHeader.textContent = assistantHeaderText(
        { driver: "human", participant_name: humanParticipantName, model: currentModel },
        { model: currentModel }
    );
    if (window._setActiveStreamMessage) {
        window._setActiveStreamMessage(assistantContent);
    }
    const reasoningPre = ensureReasoningBlock(messageDiv, { open: true });
    if (window._setActiveStreamReasoning) {
        window._setActiveStreamReasoning(reasoningPre);
    }

    try {
        const sendFn = window.go.main.App.SendMessageStream || window.go.main.App.SendMessage;
        const response = await sendFn(message, currentModel, currentAgent);
        assistantContent.textContent = response.content;
        if (response.reasoning) {
            reasoningPre.textContent = response.reasoning;
        }
        finalizeAssistantMessage(messageDiv, response);
    } catch (error) {
        console.error("Failed to send message:", error);
        assistantContent.textContent = "Error: " + error;
    } finally {
        pendingHumanTurn = false;
        if (window._setActiveStreamMessage) {
            window._setActiveStreamMessage(null);
        }
        if (window._setActiveStreamReasoning) {
            window._setActiveStreamReasoning(null);
        }
        const emptyReasoning = messageDiv.querySelector(".reasoning-details .reasoning-content");
        if (emptyReasoning && !emptyReasoning.textContent.trim()) {
            emptyReasoning.closest(".reasoning-details")?.remove();
        } else {
            const details = messageDiv.querySelector(".reasoning-details");
            if (details) {
                details.open = false;
            }
        }
    }
}

function addMessageShell(role, driver, contentElement, externalAgent = false) {
    const welcome = chatTranscript.querySelector(".welcome-message");
    if (welcome) {
        welcome.remove();
    }

    const messageDiv = document.createElement("div");
    messageDiv.className = `message ${role}`;
    if (externalAgent) {
        messageDiv.classList.add("external-agent");
    }
    messageDiv.dataset.participant = driver;

    const header = document.createElement("div");
    header.className = "message-header";
    const metadata = { driver: role === "assistant" ? "human" : driver, participant_name: driver, model: currentModel };
    header.textContent = role === "assistant"
        ? assistantHeaderText(metadata, { model: currentModel })
        : userHeaderText(metadata);

    messageDiv.appendChild(header);
    messageDiv.appendChild(contentElement);
    chatTranscript.appendChild(messageDiv);
    chatTranscript.scrollTop = chatTranscript.scrollHeight;
    return messageDiv;
}

function finalizeAssistantMessage(messageDiv, response) {
    const header = messageDiv.querySelector(".message-header");
    header.textContent = assistantHeaderText(
        { driver: "human", participant_name: humanParticipantName, model: currentModel },
        { model: currentModel, latency: response.latency, tokens: response.tokens }
    );

    if (response.system_prompt) {
        const details = document.createElement("details");
        details.className = "system-prompt-details";
        const summary = document.createElement("summary");
        summary.textContent = "System prompt used";
        const pre = document.createElement("pre");
        pre.textContent = response.system_prompt;
        details.appendChild(summary);
        details.appendChild(pre);
        messageDiv.appendChild(details);
    }

    if (response.tokens) {
        tokenBadge.textContent = `Tokens: ${response.tokens}`;
    }
    if (response.latency) {
        latencyBadge.textContent = `Latency: ${response.latency}ms`;
    }
}

function addMessageToTranscript(role, content, driver, latency = 0, tokens = 0) {
    // Remove welcome message if present
    const welcome = chatTranscript.querySelector(".welcome-message");
    if (welcome) {
        welcome.remove();
    }

    const messageDiv = document.createElement("div");
    messageDiv.className = `message ${role}`;

    const header = document.createElement("div");
    header.className = "message-header";
    header.textContent = role === "assistant"
        ? assistantHeaderText({ driver, participant_name: driver, model: currentModel }, { model: currentModel })
        : userHeaderText({ driver, participant_name: driver });

    const contentDiv = document.createElement("div");
    contentDiv.className = "message-content";
    contentDiv.textContent = content;

    messageDiv.appendChild(header);
    messageDiv.appendChild(contentDiv);
    chatTranscript.appendChild(messageDiv);

    // Scroll to bottom
    chatTranscript.scrollTop = chatTranscript.scrollHeight;

    // Update badges
    if (tokens) {
        tokenBadge.textContent = `Tokens: ${tokens}`;
    }
    if (latency) {
        latencyBadge.textContent = `Latency: ${latency}ms`;
    }
}

function updateSystemPromptBadge() {
    if (!window.go?.main?.App?.GetSystemPrompt) {
        return;
    }
    window.go.main.App.GetSystemPrompt(currentAgent)
        .then((prompt) => {
            const approxTokens = Math.ceil(prompt.length / 4);
            tokenBadge.textContent = `System prompt ~${approxTokens} tokens`;
        })
        .catch((error) => {
            console.error("Failed to load system prompt:", error);
        });
}
