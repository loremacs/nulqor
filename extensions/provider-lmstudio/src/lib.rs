//! LM Studio provider extension — Phase 2.1 (BUILD_PLAN §2.1, decisions/006 §1).
//!
//! Provides slotted capability `provider`/`lmstudio`, satisfying `provider@1`.
//! Owns the single-flight generation queue so concurrent callers wait their turn.
//!
//! generate flow:
//!   1. Command returns `{ stream_id }` immediately.
//!   2. Background task acquires the mutex, streams SSE from LM Studio.
//!   3. Events emitted: stream-start → N×stream-delta → stream-done (or stream-error).

use std::sync::{Arc, RwLock};

use tokio::sync::Mutex;

use std::time::Duration;

use crate::capability::Capabilities;
use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::runtime::Runtime;
use crate::types::{CommandDecl, CommandId, EventId, ExtensionManifest, NamespacedEvent, Permission};

// ---------------------------------------------------------------------------
// Shared provider state
// ---------------------------------------------------------------------------

pub struct ProviderState {
    /// LM Studio base URL, e.g. `http://localhost:1234/v1`.
    pub base_url: RwLock<String>,
    /// Single-flight gate — one generation at a time.
    pub generation_lock: Mutex<()>,
    /// Pooled HTTP client.
    pub http: reqwest::Client,
    /// Last auto-selected model id (first from /v1/models on connect).
    pub active_model: RwLock<Option<String>>,
}

impl ProviderState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base_url: RwLock::new("http://localhost:1234/v1".into()),
            generation_lock: Mutex::new(()),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("reqwest client"),
            active_model: RwLock::new(None),
        })
    }
}// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

pub struct LmStudioProvider {
    manifest: ExtensionManifest,
    state: Arc<ProviderState>,
}

impl LmStudioProvider {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest, state: ProviderState::new() }
    }
}

impl Extension for LmStudioProvider {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        register_connect(self.state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
        register_models(self.state.clone(), ctx.runtime.clone(), &ctx.commands)?;
        register_generate(self.state.clone(), ctx.bus.clone(), ctx.runtime.clone(), ctx.commands.clone())?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Command registrations
// ---------------------------------------------------------------------------

fn register_connect(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "provider".into(), action: "connect".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: r#"{ "url": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "model_count": "number", "models": "array" }"#
                .into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let url = input["url"]
                .as_str()
                .ok_or_else(|| CoreError::Io("connect: 'url' field required".into()))?
                .trim_end_matches('/')
                .to_owned();

            caps.check_http_allowed("provider-lmstudio", &url)?;

            let base = format!("{}/v1", url.trim_end_matches("/v1"));
            *state.base_url.write().unwrap() = base.clone();

            let models = fetch_models_sync(&runtime, &state.http, &base)?;
            let count = models.len();
            if let Some(first) = models.first() {
                *state.active_model.write().unwrap() = Some(first.clone());
            }

            Ok(serde_json::json!({
                "ok": true,
                "model_count": count,
                "models": models,
            }))
        }),
    )
}

fn register_models(
    state: Arc<ProviderState>,
    runtime: Arc<Runtime>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "provider".into(), action: "models".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "models": "array", "active": "string|null" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let base = state.base_url.read().unwrap().clone();
            let models = fetch_models_sync(&runtime, &state.http, &base)?;
            let active = state.active_model.read().unwrap().clone();
            Ok(serde_json::json!({ "models": models, "active": active }))
        }),
    )
}

fn register_generate(
    state: Arc<ProviderState>,
    bus: Arc<EventBus>,
    runtime: Arc<Runtime>,
    cmds: Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "provider".into(), action: "generate".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: r#"{ "messages": "array", "model": "string?", "agent": "string?", "system_prompt": "string?", "temperature": "number?", "max_tokens": "number?" }"#.into(),
            output_schema: r#"{ "stream_id": "string" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let state = state.clone();
            let bus = bus.clone();
            let sid = uuid::Uuid::new_v4().to_string();
            let sid_out = sid.clone();

            let system_prompt = resolve_system_prompt(&cmds_for_handler, &input);
            let mut prepared = input.clone();
            prepared["messages"] = build_chat_messages(&input, system_prompt.as_deref());

            // Return stream_id immediately; generation runs in background.
            runtime.spawn_task(Duration::from_secs(300), async move {
                do_generate(state, bus, sid, prepared).await;
            });

            Ok(serde_json::json!({ "stream_id": sid_out }))
        }),
    )
}

/// Resolve the system prompt for a generation call.
/// Explicit `system_prompt` input wins; otherwise fetch from context-editor.
fn resolve_system_prompt(
    cmds: &crate::commands::CommandRegistry,
    input: &serde_json::Value,
) -> Option<String> {
    if let Some(sp) = input["system_prompt"].as_str() {
        return if sp.is_empty() { None } else { Some(sp.to_owned()) };
    }

    let agent_input = match input["agent"].as_str() {
        Some(agent) => serde_json::json!({ "agent": agent }),
        None => serde_json::json!({}),
    };

    match cmds.invoke(
        "provider-lmstudio",
        &CommandId::parse("context-editor:system-prompt@1").ok()?,
        agent_input,
    ) {
        Ok(v) => {
            let prompt = v["prompt"].as_str().unwrap_or("").to_owned();
            if prompt.is_empty() { None } else { Some(prompt) }
        }
        Err(e) => {
            eprintln!("[provider-lmstudio] system-prompt unavailable: {e}");
            None
        }
    }
}

/// Build LM Studio messages: optional system prompt first, then user/assistant turns.
fn build_chat_messages(input: &serde_json::Value, system_prompt: Option<&str>) -> serde_json::Value {
    let raw = input["messages"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();

    if let Some(sp) = system_prompt {
        if !sp.is_empty() {
            let has_system =
                raw.first().and_then(|m| m["role"].as_str()) == Some("system");
            if !has_system {
                out.push(serde_json::json!({ "role": "system", "content": sp }));
            }
        }
    }

    for m in raw {
        let Some(role) = m["role"].as_str() else { continue };
        if !matches!(role, "user" | "assistant" | "system") {
            continue;
        }
        let content = m["content"].as_str().unwrap_or("");
        out.push(serde_json::json!({ "role": role, "content": content }));
    }

    serde_json::Value::Array(out)
}

/// Fetch model ids (sync entry — safe from HTTP API async handlers).
fn fetch_models_sync(
    runtime: &Runtime,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<String>, CoreError> {
    let http = client.clone();
    let base = base_url.to_owned();
    runtime.block_on_compat(async move { fetch_models(&http, &base).await })
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

/// Fetch model ids from `/v1/models`. Never hardcodes an id (decisions/006 §11 #7).
async fn fetch_models(client: &reqwest::Client, base_url: &str) -> Result<Vec<String>, CoreError> {
    let url = format!("{}/models", base_url);
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;

    let json: serde_json::Value =
        resp.json().await.map_err(|e| CoreError::Io(format!("models JSON: {e}")))?;

    Ok(json["data"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| m["id"].as_str().map(str::to_owned))
        .collect())
}

/// Streaming generation task (runs via `runtime.spawn_task`).
/// Parses LM Studio SSE, emits provider:stream-* events on the bus.
async fn do_generate(
    state: Arc<ProviderState>,
    bus: Arc<EventBus>,
    stream_id: String,
    input: serde_json::Value,
) {
    use futures_util::StreamExt;

    let emit = |name: &str, payload: serde_json::Value| {
        let _ = bus.publish(NamespacedEvent {
            id: EventId { namespace: "provider".into(), name: name.to_owned(), version: 1 },
            payload,
        });
    };

    emit("stream-start", serde_json::json!({ "stream_id": stream_id }));

    // Acquire single-flight gate
    let _guard = state.generation_lock.lock().await;

    let base_url = state.base_url.read().unwrap().clone();

    let model = input["model"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            state.active_model.try_read().ok().and_then(|g| g.clone()).unwrap_or_default()
        });

    if model.is_empty() {
        emit(
            "stream-error",
            serde_json::json!({
                "stream_id": stream_id,
                "error": "no model loaded — call provider:connect@1 first",
            }),
        );
        return;
    }

    let body = serde_json::json!({
        "model": model,
        "messages": input["messages"],
        "stream": true,
        "temperature": input["temperature"].as_f64().unwrap_or(0.7),
        "max_tokens": input["max_tokens"].as_i64().unwrap_or(2048),
    });

    let resp = match state
        .http
        .post(format!("{}/chat/completions", base_url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            emit(
                "stream-error",
                serde_json::json!({ "stream_id": stream_id, "error": e.to_string() }),
            );
            return;
        }
    };

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        emit(
            "stream-error",
            serde_json::json!({ "stream_id": stream_id, "error": format!("HTTP {status}: {text}") }),
        );
        return;
    }

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full_content = String::new();
    let mut reasoning = String::new();
    let mut tokens: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                emit(
                    "stream-error",
                    serde_json::json!({ "stream_id": stream_id, "error": e.to_string() }),
                );
                return;
            }
        };
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(nl) = buf.find('\n') {
            let line = buf[..nl].trim_end_matches('\r').to_owned();
            buf = buf[nl + 1..].to_owned();

            let Some(data) = line.strip_prefix("data: ") else { continue };
            if data == "[DONE]" {
                continue;
            }

            let Ok(json) = serde_json::from_str::<serde_json::Value>(data) else { continue };

            if let Some(delta) = json["choices"][0]["delta"]["content"].as_str() {
                full_content.push_str(delta);
                emit(
                    "stream-delta",
                    serde_json::json!({ "stream_id": stream_id, "delta": delta }),
                );
            }
            if let Some(rc) = json["choices"][0]["delta"]["reasoning_content"].as_str() {
                reasoning.push_str(rc);
            }
            if let Some(t) = json["usage"]["completion_tokens"].as_u64() {
                tokens = t;
            }
        }
    }

    emit(
        "stream-done",
        serde_json::json!({
            "stream_id": stream_id,
            "content": full_content,
            "reasoning": if reasoning.is_empty() { serde_json::Value::Null } else { serde_json::json!(reasoning) },
            "tokens": tokens,
            "model": model,
        }),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_context() -> CoreContext {
        use crate::{
            capability::{CapabilityRegistry, Capabilities},
            commands::CommandRegistry,
            context::InMemoryConfigStore,
            events::EventBus,
            permission::PermissionGate,
            version::VersionManager,
        };
        let perms = Arc::new(PermissionGate::new());
        CoreContext {
            bus: Arc::new(EventBus::new()),
            commands: Arc::new(CommandRegistry::new(perms.clone())),
            versions: Arc::new(VersionManager::new()),
            permissions: perms,
            caps: Arc::new(Capabilities::new()),
            capability_registry: Arc::new(CapabilityRegistry::new()),
            runtime: Arc::new(Runtime::new()),
            config: Arc::new(InMemoryConfigStore::new()),
        }
    }

    fn make_manifest() -> ExtensionManifest {
        use crate::types::ExtensionKind;
        ExtensionManifest {
            id: "provider-lmstudio".into(),
            version: semver::Version::parse("0.1.0").unwrap(),
            kind: ExtensionKind::Provider,
            api_version: "v1".into(),
            schema_version: semver::Version::parse("1.0.0").unwrap(),
            min_core: semver::Version::parse("0.1.0").unwrap(),
            requires: vec![],
            optional: vec![],
            provides: vec![],
            commands: vec![],
            publishes: vec![],
            subscribes: vec![],
            fs_scopes: vec![],
            http_hosts: vec!["localhost".into()],
        }
    }

    #[test]
    fn provider_registers_three_commands() {
        let ctx = make_context();
        // Register scopes so http_allowed check works
        ctx.caps.register_scopes("provider-lmstudio", vec![], vec!["localhost".into()]);

        let ext = LmStudioProvider::new(make_manifest());
        ext.activate(&ctx).expect("activate should succeed");

        let cmds = ctx.commands.list_commands();
        assert!(cmds.iter().any(|c| c == "provider:connect@1"), "connect not registered: {cmds:?}");
        assert!(cmds.iter().any(|c| c == "provider:models@1"), "models not registered: {cmds:?}");
        assert!(cmds.iter().any(|c| c == "provider:generate@1"), "generate not registered: {cmds:?}");
    }

    #[test]
    fn connect_fails_on_missing_url_field() {
        let ctx = make_context();
        ctx.caps.register_scopes("provider-lmstudio", vec![], vec!["localhost".into()]);

        let ext = LmStudioProvider::new(make_manifest());
        ext.activate(&ctx).unwrap();

        // Invoke connect with no `url` field → should fail with Io error
        let result = ctx.commands.invoke(
            "test",
            &crate::types::CommandId::parse("provider:connect@1").unwrap(),
            serde_json::json!({}),
        );
        assert!(result.is_err(), "expected error for missing url field");
    }

    #[test]
    fn connect_rejects_non_localhost_url() {
        let ctx = make_context();
        // Register only localhost as allowed host
        ctx.caps.register_scopes("provider-lmstudio", vec![], vec!["localhost".into()]);

        let ext = LmStudioProvider::new(make_manifest());
        ext.activate(&ctx).unwrap();

        // Attempt to connect to an external host — BoundaryViolation expected
        let result = ctx.commands.invoke(
            "test",
            &crate::types::CommandId::parse("provider:connect@1").unwrap(),
            serde_json::json!({ "url": "http://evil.example.com" }),
        );
        assert!(
            matches!(result, Err(crate::error::CoreError::BoundaryViolation(_))),
            "expected BoundaryViolation, got: {result:?}"
        );
    }

    #[test]
    fn build_chat_messages_prepends_system_prompt() {
        let input = serde_json::json!({
            "messages": [
                { "role": "user", "content": "hello", "id": "x" },
            ]
        });
        let built = build_chat_messages(&input, Some("Current date: 2026-05-24"));
        let arr = built.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["role"], "system");
        assert!(arr[0]["content"].as_str().unwrap().contains("2026-05-24"));
        assert_eq!(arr[1]["role"], "user");
    }

    #[test]
    fn build_chat_messages_skips_duplicate_system() {
        let input = serde_json::json!({
            "messages": [
                { "role": "system", "content": "existing" },
                { "role": "user", "content": "hi" },
            ]
        });
        let built = build_chat_messages(&input, Some("new system"));
        let arr = built.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["content"], "existing");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn generate_returns_stream_id_immediately() {
        let ctx = make_context();
        ctx.caps.register_scopes("provider-lmstudio", vec![], vec!["localhost".into()]);

        let ext = LmStudioProvider::new(make_manifest());
        ext.activate(&ctx).unwrap();

        // generate without a loaded model — returns stream_id, background task emits stream-error
        let result = ctx.commands.invoke(
            "test",
            &crate::types::CommandId::parse("provider:generate@1").unwrap(),
            serde_json::json!({ "messages": [] }),
        );
        assert!(result.is_ok(), "generate should return Ok immediately: {result:?}");
        let val = result.unwrap();
        assert!(val["stream_id"].is_string(), "should have stream_id: {val}");
    }
}
