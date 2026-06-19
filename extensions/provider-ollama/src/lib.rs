//! Ollama provider — OpenAI-compatible `/v1` API on localhost:11434.

use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio::sync::Mutex;

use crate::capability::Capabilities;
use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::extensions::provider_common::{
    build_chat_messages, fetch_openai_models, model_ids_match, openai_base, ping_openai_server,
    resolve_system_prompt, server_root, stream_openai_chat, dedupe_preserve, HttpClients,
};
use crate::runtime::Runtime;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

const EXT_ID: &str = "provider-ollama";
const NS: &str = "ollama";

pub struct ProviderState {
    pub base_url: RwLock<String>,
    pub cached_models: RwLock<Vec<String>>,
    pub connected: RwLock<bool>,
    pub generation_lock: Mutex<()>,
    pub http: HttpClients,
    pub active_model: RwLock<Option<String>>,
    nulqor_warmed: RwLock<Vec<String>>,
}

impl ProviderState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base_url: RwLock::new("http://localhost:11434/v1".into()),
            cached_models: RwLock::new(Vec::new()),
            connected: RwLock::new(false),
            generation_lock: Mutex::new(()),
            http: HttpClients::new(),
            active_model: RwLock::new(None),
            nulqor_warmed: RwLock::new(Vec::new()),
        })
    }
}

pub struct OllamaProvider {
    manifest: ExtensionManifest,
    state: Arc<ProviderState>,
}

impl OllamaProvider {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest, state: ProviderState::new() }
    }
}

impl Extension for OllamaProvider {
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
    register_disconnect(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_select_model(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_stop_model(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_models(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_loaded_models(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
    register_unload_model(state.clone(), ctx.caps.clone(), ctx.runtime.clone(), &ctx.commands)?;
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

fn nulqor_loaded_active(state: &ProviderState) -> bool {
    let active = state.active_model.read().unwrap().clone();
    let Some(model) = active else {
        return false;
    };
    state
        .nulqor_warmed
        .read()
        .unwrap()
        .iter()
        .any(|m| model_ids_match(m, &model))
}

fn track_ollama_warmed(state: &ProviderState, model: &str) {
    let mut warmed = state.nulqor_warmed.write().unwrap();
    warmed.retain(|m| !model_ids_match(m, model));
    warmed.push(model.to_owned());
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

            let warmed = warm_model_sync(&state, &caps, &runtime, &model)?;
            *state.connected.write().unwrap() = true;
            *state.active_model.write().unwrap() = Some(model.clone());

            Ok(serde_json::json!({
                "ok": true,
                "connected": true,
                "active": model,
                "nulqor_loaded": warmed,
            }))
        }),
    )
}

fn register_disconnect(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
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
            unload_all_nulqor_sync(&state, &caps, &runtime);
            *state.connected.write().unwrap() = false;
            *state.active_model.write().unwrap() = None;
            Ok(serde_json::json!({ "ok": true }))
        }),
    )
}

fn register_select_model(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
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
            let warmed = warm_model_sync(&state, &caps, &runtime, &model)?;
            *state.active_model.write().unwrap() = Some(model.clone());
            Ok(serde_json::json!({
                "ok": true,
                "active": model,
                "nulqor_loaded": warmed,
            }))
        }),
    )
}

fn register_stop_model(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
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
            let active = state.active_model.read().unwrap().clone();
            let Some(model) = active else {
                return Ok(serde_json::json!({
                    "ok": true,
                    "stopped": false,
                    "active": serde_json::Value::Null,
                }));
            };
            let stopped = unload_model_sync(&state, &caps, &runtime, &model);
            if stopped {
                *state.active_model.write().unwrap() = None;
                *state.connected.write().unwrap() = false;
            }
            Ok(serde_json::json!({
                "ok": true,
                "stopped": stopped,
                "active": if stopped { serde_json::Value::Null } else { serde_json::json!(model) },
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
                let root = server_root(&base);
                caps.check_http_allowed(EXT_ID, &root)?;
                match fetch_catalog_sync(&runtime, &state.http.probe, &base, &root) {
                    Ok(catalog) => *state.cached_models.write().unwrap() = catalog,
                    Err(e) => return Err(e),
                }
            }

            let catalog = state.cached_models.read().unwrap().clone();
            Ok(serde_json::json!({
                "models": catalog,
                "active": active,
                "connected": connected,
                "nulqor_loaded_active": nulqor_loaded_active(&state),
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
            let base = state.base_url.read().unwrap().clone();
            let root = server_root(&base);
            caps.check_http_allowed(EXT_ID, &root)?;
            let warmed = state.nulqor_warmed.read().unwrap().clone();
            let running = fetch_ollama_ps_sync(&runtime, &state.http.probe, &root)?;
            let loaded: Vec<serde_json::Value> = running
                .into_iter()
                .map(|name| {
                    let nulqor_owned = warmed.iter().any(|m| model_ids_match(m, &name));
                    loaded_entry(&name, nulqor_owned, None, true)
                })
                .collect();
            Ok(serde_json::json!({ "loaded": loaded }))
        }),
    )
}

fn register_unload_model(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
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
            let model = input["model"]
                .as_str()
                .ok_or_else(|| CoreError::Io("unload-model: 'model' field required".into()))?
                .trim()
                .to_owned();
            let stopped = unload_model_sync(&state, &caps, &runtime, &model);
            if stopped {
                let active = state.active_model.read().unwrap().clone();
                if active.as_deref() == Some(model.as_str()) {
                    *state.active_model.write().unwrap() = None;
                    *state.connected.write().unwrap() = false;
                }
            }
            Ok(serde_json::json!({ "ok": true, "stopped": stopped }))
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

async fn fetch_ollama_ps(client: &reqwest::Client, root: &str) -> Result<Vec<String>, CoreError> {
    let url = format!("{}/api/ps", root.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;
    let json: serde_json::Value =
        resp.json().await.map_err(|e| CoreError::Io(format!("ps JSON: {e}")))?;
    Ok(json["models"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| {
            m["name"]
                .as_str()
                .or_else(|| m["model"].as_str())
                .map(str::to_owned)
        })
        .collect())
}

fn fetch_ollama_ps_sync(
    runtime: &Runtime,
    client: &reqwest::Client,
    root: &str,
) -> Result<Vec<String>, CoreError> {
    let http = client.clone();
    let root = root.to_owned();
    runtime.block_on_compat(async move { fetch_ollama_ps(&http, &root).await })
}

async fn fetch_ollama_tags(client: &reqwest::Client, root: &str) -> Result<Vec<String>, CoreError> {
    let url = format!("{}/api/tags", root.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;
    let json: serde_json::Value =
        resp.json().await.map_err(|e| CoreError::Io(format!("tags JSON: {e}")))?;
    Ok(json["models"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|m| m["name"].as_str().map(str::to_owned))
        .collect())
}

fn fetch_catalog_sync(
    runtime: &Runtime,
    client: &reqwest::Client,
    base: &str,
    root: &str,
) -> Result<Vec<String>, CoreError> {
    let http = client.clone();
    let base = base.to_owned();
    let root = root.to_owned();
    runtime.block_on_compat(async move {
        let openai = fetch_openai_models(&http, &base).await.unwrap_or_default();
        if !openai.is_empty() {
            return Ok(dedupe_preserve(openai));
        }
        fetch_ollama_tags(&http, &root).await
    })
}

async fn warm_model(client: &reqwest::Client, root: &str, model: &str) -> Result<(), CoreError> {
    let url = format!("{}/api/generate", root.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "model": model,
            "prompt": "",
            "stream": false,
            "keep_alive": "5m",
        }))
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("POST {url}: {e}")))?;
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(CoreError::Io(format!(
        "warm model '{model}' failed ({status}): {body}"
    )))
}

async fn unload_model(client: &reqwest::Client, root: &str, model: &str) -> Result<(), CoreError> {
    let url = format!("{}/api/generate", root.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&serde_json::json!({
            "model": model,
            "keep_alive": 0,
        }))
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("POST {url}: {e}")))?;
    if resp.status().is_success() {
        return Ok(());
    }
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(CoreError::Io(format!(
        "unload model '{model}' failed ({status}): {body}"
    )))
}

fn warm_model_sync(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
    model: &str,
) -> Result<bool, CoreError> {
    let base = state.base_url.read().unwrap().clone();
    let root = server_root(&base);
    caps.check_http_allowed(EXT_ID, &root)?;

    let http_probe = state.http.probe.clone();
    let root_clone = root.clone();
    let model_name = model.to_owned();
    let already_running = runtime.block_on_compat(async move {
        fetch_ollama_ps(&http_probe, &root_clone)
            .await
            .map(|names| names.iter().any(|n| model_ids_match(n, &model_name)))
            .unwrap_or(false)
    });

    if already_running {
        *state.active_model.write().unwrap() = Some(model.to_owned());
        track_ollama_warmed(state, model);
        return Ok(true);
    }

    let http = state.http.load.clone();
    let root = root.clone();
    let model_name = model.to_owned();
    runtime.block_on_compat(async move { warm_model(&http, &root, &model_name).await })?;

    track_ollama_warmed(state, model);
    Ok(true)
}

fn unload_model_sync(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
    model: &str,
) -> bool {
    let in_list = state.nulqor_warmed.read().unwrap().iter().any(|m| m == model);
    if !in_list {
        return false;
    }
    let base = state.base_url.read().unwrap().clone();
    let root = server_root(&base);
    if caps.check_http_allowed(EXT_ID, &root).is_err() {
        return false;
    }
    let http = state.http.load.clone();
    let model_owned = model.to_owned();
    let result = runtime
        .block_on_compat(async move { unload_model(&http, &root, &model_owned).await });
    match result {
        Ok(()) => {
            let mut warmed = state.nulqor_warmed.write().unwrap();
            warmed.retain(|m| m != model);
            true
        }
        Err(e) => {
            eprintln!("[provider-ollama] unload failed: {e}");
            false
        }
    }
}

fn unload_all_nulqor_sync(state: &ProviderState, caps: &Capabilities, runtime: &Runtime) {
    let models: Vec<String> = state.nulqor_warmed.read().unwrap().clone();
    for model in models {
        if !unload_model_sync(state, caps, runtime, &model) {
            eprintln!("[nulqor] Failed to unload model: {:?}", model);
        }
    }
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
        "no model selected — connect to Ollama and pick a model",
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nulqor_loaded_active_tracks_warmed_models() {
        let state = ProviderState::new();
        *state.active_model.write().unwrap() = Some("llama3.2".into());
        *state.nulqor_warmed.write().unwrap() = vec!["llama3.2".into()];
        assert!(nulqor_loaded_active(&state));
    }
}
