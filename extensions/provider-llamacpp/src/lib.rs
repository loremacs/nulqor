//! llama.cpp server provider — OpenAI-compatible `/v1` on localhost:8080.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::Mutex;

use crate::capability::Capabilities;
use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::extensions::provider_common::{
    build_chat_messages, fetch_openai_models, openai_base, ping_openai_server,
    resolve_system_prompt, stream_openai_chat, dedupe_preserve, HttpClients,
};
use crate::runtime::Runtime;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

const EXT_ID: &str = "provider-llamacpp";
const NS: &str = "llamacpp";

pub struct ProviderState {
    pub base_url: RwLock<String>,
    pub cached_models: RwLock<Vec<String>>,
    pub connected: RwLock<bool>,
    pub generation_lock: Mutex<()>,
    pub http: HttpClients,
    pub active_model: RwLock<Option<String>>,
}

impl ProviderState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base_url: RwLock::new("http://localhost:8080/v1".into()),
            cached_models: RwLock::new(Vec::new()),
            connected: RwLock::new(false),
            generation_lock: Mutex::new(()),
            http: HttpClients::new(),
            active_model: RwLock::new(None),
        })
    }
}

pub struct LlamaCppProvider {
    manifest: ExtensionManifest,
    state: Arc<ProviderState>,
}

impl LlamaCppProvider {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest, state: ProviderState::new() }
    }
}

impl Extension for LlamaCppProvider {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        register_all(self.state.clone(), ctx)?;
        Ok(())
    }
}

fn register_all(state: Arc<ProviderState>, ctx: &CoreContext) -> Result<(), CoreError> {
    register_connect(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_disconnect(state.clone(), &ctx.commands)?;
    register_select_model(state.clone(), &ctx.commands)?;
    register_stop_model(state.clone(), &ctx.commands)?;
    register_models(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_loaded_models(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_unload_model(state.clone(), &ctx.commands)?;
    register_generate(
        state,
        ctx.bus.clone(),
        ctx.runtime.clone(),
        ctx.commands.clone(),
    )?;
    Ok(())
}

fn cmd_id(action: &str) -> CommandId {
    CommandId { namespace: NS.into(), action: action.into(), version: 1 }
}

fn register_connect(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: cmd_id("connect"),
            owner: EXT_ID.into(),
            input_schema: r#"{ "url": "string", "model": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "connected": "boolean", "active": "string", "nulqor_loaded": "boolean" }"#
                .into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let url = input["url"]
                .as_str()
                .ok_or_else(|| CoreError::Io("connect: 'url' field required".into()))?;
            let model = input["model"]
                .as_str()
                .ok_or_else(|| CoreError::Io("connect: 'model' field required".into()))?
                .trim()
                .to_owned();
            if model.is_empty() {
                return Err(CoreError::Io("connect: 'model' must not be empty".into()));
            }

            caps.check_http_allowed(EXT_ID, url)?;
            let base = openai_base(url);
            ping_sync(&runtime, &state.http.probe, &base)?;
            *state.base_url.write().unwrap() = base;
            *state.connected.write().unwrap() = true;
            *state.active_model.write().unwrap() = Some(model.clone());

            Ok(serde_json::json!({
                "ok": true,
                "connected": true,
                "active": model,
                "nulqor_loaded": false,
            }))
        }),
    )
}

fn register_disconnect(
    state: Arc<ProviderState>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: cmd_id("disconnect"),
            owner: EXT_ID.into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "ok": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            *state.connected.write().unwrap() = false;
            *state.active_model.write().unwrap() = None;
            Ok(serde_json::json!({ "ok": true }))
        }),
    )
}

fn register_select_model(
    state: Arc<ProviderState>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: cmd_id("select-model"),
            owner: EXT_ID.into(),
            input_schema: r#"{ "model": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "active": "string", "nulqor_loaded": "boolean" }"#
                .into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let model = input["model"]
                .as_str()
                .ok_or_else(|| CoreError::Io("select-model: 'model' required".into()))?
                .trim()
                .to_owned();
            *state.active_model.write().unwrap() = Some(model.clone());
            Ok(serde_json::json!({
                "ok": true,
                "active": model,
                "nulqor_loaded": false,
            }))
        }),
    )
}

fn register_stop_model(
    state: Arc<ProviderState>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: cmd_id("stop-model"),
            owner: EXT_ID.into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "ok": "boolean", "stopped": "boolean", "active": "string|null" }"#
                .into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let had = state.active_model.read().unwrap().is_some();
            *state.active_model.write().unwrap() = None;
            *state.connected.write().unwrap() = false;
            Ok(serde_json::json!({
                "ok": true,
                "stopped": had,
                "active": serde_json::Value::Null,
            }))
        }),
    )
}

fn register_models(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: cmd_id("models"),
            owner: EXT_ID.into(),
            input_schema: r#"{ "refresh": "boolean?", "url": "string?" }"#.into(),
            output_schema: r#"{ "models": "array", "active": "string|null", "connected": "boolean", "nulqor_loaded_active": "boolean" }"#
                .into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let refresh = input["refresh"].as_bool().unwrap_or(false);
            let connected = *state.connected.read().unwrap();
            let active = state.active_model.read().unwrap().clone();

            if refresh {
                if let Some(url) = input["url"].as_str() {
                    caps.check_http_allowed(EXT_ID, url)?;
                    *state.base_url.write().unwrap() = openai_base(url);
                }
                let base = state.base_url.read().unwrap().clone();
                caps.check_http_allowed(EXT_ID, &base)?;
                match fetch_models_sync(&runtime, &state.http.probe, &base) {
                    Ok(catalog) => *state.cached_models.write().unwrap() = catalog,
                    Err(e) => return Err(e),
                }
            }

            let catalog = state.cached_models.read().unwrap().clone();
            Ok(serde_json::json!({
                "models": catalog,
                "active": active,
                "connected": connected,
                "nulqor_loaded_active": false,
            }))
        }),
    )
}

fn register_loaded_models(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    use crate::extensions::provider_common::loaded_entry;

    cmds.register(
        CommandDecl {
            id: cmd_id("loaded-models"),
            owner: EXT_ID.into(),
            input_schema: r#"{ "refresh": "boolean?", "url": "string?" }"#.into(),
            output_schema: r#"{ "loaded": "array" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            if input.get("refresh").and_then(|v| v.as_bool()).unwrap_or(true) {
                if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                    caps.check_http_allowed(EXT_ID, url)?;
                    *state.base_url.write().unwrap() = openai_base(url);
                }
            }
            let connected = *state.connected.read().unwrap();
            let active = state.active_model.read().unwrap().clone();
            let loaded = if connected {
                active
                    .as_ref()
                    .map(|model| vec![loaded_entry(model, false, None, false)])
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            let _ = runtime;
            Ok(serde_json::json!({ "loaded": loaded }))
        }),
    )
}

fn register_unload_model(
    state: Arc<ProviderState>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: cmd_id("unload-model"),
            owner: EXT_ID.into(),
            input_schema: r#"{ "model": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "stopped": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let model = input["model"].as_str().unwrap_or("").trim();
            let active = state.active_model.read().unwrap().clone();
            let matches = active.as_deref() == Some(model);
            Ok(serde_json::json!({
                "ok": true,
                "stopped": false,
                "reason": if matches {
                    "llama.cpp unloads only when the server restarts"
                } else {
                    "model not active in Nulqor"
                },
            }))
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
            id: cmd_id("generate"),
            owner: EXT_ID.into(),
            input_schema: r#"{ "messages": "array", "model": "string?", "agent": "string?", "system_prompt": "string?", "temperature": "number?", "max_tokens": "number?" }"#
                .into(),
            output_schema: r#"{ "stream_id": "string" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let sid = uuid::Uuid::new_v4().to_string();
            let sid_out = sid.clone();
            let system_prompt = resolve_system_prompt(EXT_ID, &cmds_for_handler, &input);
            let mut prepared = input.clone();
            prepared["messages"] = build_chat_messages(&input, system_prompt.as_deref());
            let state = state.clone();
            let bus = bus.clone();
            runtime.spawn_task(Duration::from_secs(300), async move {
                do_generate(state, bus, sid, prepared).await;
            });
            Ok(serde_json::json!({ "stream_id": sid_out }))
        }),
    )
}

fn ping_sync(runtime: &Runtime, client: &reqwest::Client, base: &str) -> Result<(), CoreError> {
    let http = client.clone();
    let base = base.to_owned();
    runtime.block_on_compat(async move { ping_openai_server(&http, &base).await })
}

fn fetch_models_sync(
    runtime: &Runtime,
    client: &reqwest::Client,
    base: &str,
) -> Result<Vec<String>, CoreError> {
    let http = client.clone();
    let base = base.to_owned();
    runtime.block_on_compat(async move {
        let ids = fetch_openai_models(&http, &base).await?;
        Ok(dedupe_preserve(ids))
    })
}

async fn do_generate(
    state: Arc<ProviderState>,
    bus: Arc<EventBus>,
    stream_id: String,
    input: serde_json::Value,
) {
    let _guard = state.generation_lock.lock().await;
    let base_url = state.base_url.read().unwrap().clone();
    let model = input["model"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            state
                .active_model
                .read()
                .ok()
                .and_then(|g| g.clone())
                .unwrap_or_default()
        });

    if let Ok(mut active) = state.active_model.write() {
        if !model.is_empty() {
            *active = Some(model.clone());
        }
    }

    stream_openai_chat(
        &bus,
        &stream_id,
        &state.http.generate,
        &base_url,
        &model,
        &input["messages"],
        input["temperature"].as_f64().unwrap_or(0.7),
        input["max_tokens"].as_i64().unwrap_or(2048),
        "no model selected — connect to llama.cpp server and pick a model",
    )
    .await;
}
