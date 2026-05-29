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
use crate::extensions::provider_common::model_ids_match;
use crate::types::{CommandDecl, CommandId, EventId, ExtensionManifest, NamespacedEvent, Permission};

#[derive(Clone, Debug, PartialEq, Eq)]
struct NulqorLoadedInstance {
    model: String,
    instance_id: String,
}

// ---------------------------------------------------------------------------
// Shared provider state
// ---------------------------------------------------------------------------

pub struct ProviderState {
    /// LM Studio base URL, e.g. `http://localhost:1234/v1`.
    pub base_url: RwLock<String>,
    /// Last model list from a successful connect / refresh.
    pub cached_models: RwLock<Vec<String>>,
    /// Whether the last connect succeeded (server reachable).
    pub connected: RwLock<bool>,
    /// Single-flight gate — one generation at a time.
    pub generation_lock: Mutex<()>,
    /// Short-timeout client for connect / model probes.
    pub http_probe: reqwest::Client,
    /// Long-timeout client for model load + streaming chat completions.
    pub http_load: reqwest::Client,
    /// Long-timeout client for streaming chat completions.
    pub http_generate: reqwest::Client,
    /// Selected model id for generate (may differ from LM Studio GUI selection).
    pub active_model: RwLock<Option<String>>,
    /// Model instances loaded by this Nulqor process — only these are unloaded on stop/disconnect.
    nulqor_loaded: RwLock<Vec<NulqorLoadedInstance>>,
}

impl ProviderState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            base_url: RwLock::new("http://localhost:1234/v1".into()),
            cached_models: RwLock::new(Vec::new()),
            connected: RwLock::new(false),
            generation_lock: Mutex::new(()),
            http_probe: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .connect_timeout(Duration::from_secs(2))
                .build()
                .expect("reqwest probe client"),
            http_load: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest load client"),
            http_generate: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .expect("reqwest generate client"),
            active_model: RwLock::new(None),
            nulqor_loaded: RwLock::new(Vec::new()),
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
        register_disconnect(
            self.state.clone(),
            ctx.caps.clone(),
            ctx.runtime.clone(),
            &ctx.commands,
        )?;
        register_select_model(
            self.state.clone(),
            ctx.caps.clone(),
            ctx.runtime.clone(),
            &ctx.commands,
        )?;
        register_stop_model(
            self.state.clone(),
            ctx.caps.clone(),
            ctx.runtime.clone(),
            &ctx.commands,
        )?;
        register_models(
            self.state.clone(),
            ctx.caps.clone(),
            ctx.runtime.clone(),
            &ctx.commands,
        )?;
        register_loaded_models(
            self.state.clone(),
            ctx.caps.clone(),
            ctx.runtime.clone(),
            &ctx.commands,
        )?;
        register_unload_model(
            self.state.clone(),
            ctx.caps.clone(),
            ctx.runtime.clone(),
            &ctx.commands,
        )?;
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
            id: CommandId { namespace: "lmstudio".into(), action: "connect".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: r#"{ "url": "string", "model": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "connected": "boolean", "active": "string", "nulqor_loaded": "boolean" }"#
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
            let model = input["model"]
                .as_str()
                .ok_or_else(|| CoreError::Io("connect: 'model' field required".into()))?
                .trim()
                .to_owned();
            if model.is_empty() {
                return Err(CoreError::Io("connect: 'model' must not be empty".into()));
            }

            caps.check_http_allowed("provider-lmstudio", &url)?;

            let base = format!("{}/v1", url.trim_end_matches("/v1"));
            ping_server_sync(&runtime, &state.http_probe, &base)?;
            *state.base_url.write().unwrap() = base;

            let nulqor_loaded = spin_up_model(&state, &caps, &runtime, &model)?;
            *state.connected.write().unwrap() = true;

            Ok(serde_json::json!({
                "ok": true,
                "connected": true,
                "active": model,
                "nulqor_loaded": nulqor_loaded,
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
            id: CommandId { namespace: "lmstudio".into(), action: "disconnect".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "ok": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            unload_all_nulqor_sync(&state, &caps, &runtime);
            *state.active_model.write().unwrap() = None;
            *state.connected.write().unwrap() = false;
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
            id: CommandId { namespace: "lmstudio".into(), action: "select-model".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: r#"{ "model": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean", "active": "string", "nulqor_loaded": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let model = input["model"]
                .as_str()
                .ok_or_else(|| CoreError::Io("select-model: 'model' field required".into()))?
                .trim()
                .to_owned();
            if model.is_empty() {
                return Err(CoreError::Io("select-model: 'model' must not be empty".into()));
            }

            let nulqor_loaded = spin_up_model(&state, &caps, &runtime, &model)?;
            *state.connected.write().unwrap() = true;

            Ok(serde_json::json!({
                "ok": true,
                "active": model,
                "nulqor_loaded": nulqor_loaded,
                "connected": true,
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
            id: CommandId { namespace: "lmstudio".into(), action: "stop-model".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "ok": "boolean", "stopped": "boolean", "active": "string|null" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let active = state.active_model.read().unwrap().clone();
            let Some(model) = active else {
                return Ok(serde_json::json!({
                    "ok": true,
                    "stopped": false,
                    "active": null,
                }));
            };

            let stopped = unload_nulqor_model_sync(&state, &caps, &runtime, &model);
            if stopped {
                *state.active_model.write().unwrap() = None;
                *state.connected.write().unwrap() = false;
            }

            Ok(serde_json::json!({
                "ok": true,
                "stopped": stopped,
                "active": state.active_model.read().unwrap().clone(),
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
            id: CommandId { namespace: "lmstudio".into(), action: "models".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: r#"{ "refresh": "boolean?", "url": "string?" }"#.into(),
            output_schema: r#"{ "models": "array", "active": "string|null", "connected": "boolean", "nulqor_loaded_active": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let refresh = input.get("refresh").and_then(|v| v.as_bool()).unwrap_or(false);
            let active = state.active_model.read().unwrap().clone();
            let cached = state.cached_models.read().unwrap().clone();
            let connected = *state.connected.read().unwrap();
            let owned_active = nulqor_loaded_active(&state);

            if !refresh {
                return Ok(serde_json::json!({
                    "models": cached,
                    "active": active,
                    "connected": connected,
                    "nulqor_loaded_active": owned_active,
                }));
            }

            if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                let url = url.trim_end_matches('/').to_owned();
                caps.check_http_allowed("provider-lmstudio", &url)?;
                let base = format!("{}/v1", url.trim_end_matches("/v1"));
                ping_server_sync(&runtime, &state.http_probe, &base)?;
                *state.base_url.write().unwrap() = base;
            }

            let base = state.base_url.read().unwrap().clone();
            match fetch_model_catalog_sync(&runtime, &state.http_probe, &base) {
                Ok(catalog) => {
                    *state.cached_models.write().unwrap() = catalog.clone();
                    Ok(serde_json::json!({
                        "models": catalog,
                        "active": active,
                        "connected": connected,
                        "nulqor_loaded_active": nulqor_loaded_active(&state),
                    }))
                }
                Err(e) => Err(e),
            }
        }),
    )
}

fn register_loaded_models(
    state: Arc<ProviderState>,
    caps: Arc<Capabilities>,
    runtime: Arc<Runtime>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "lmstudio".into(), action: "loaded-models".into(), version: 1 },
            owner: "provider-lmstudio".into(),
            input_schema: r#"{ "refresh": "boolean?", "url": "string?" }"#.into(),
            output_schema: r#"{ "loaded": "array" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let refresh = input.get("refresh").and_then(|v| v.as_bool()).unwrap_or(true);
            if refresh {
                if let Some(url) = input.get("url").and_then(|v| v.as_str()) {
                    let url = url.trim_end_matches('/').to_owned();
                    caps.check_http_allowed("provider-lmstudio", &url)?;
                    let base = format!("{}/v1", url.trim_end_matches("/v1"));
                    ping_server_sync(&runtime, &state.http_probe, &base)?;
                    *state.base_url.write().unwrap() = base;
                }
            }
            let loaded = fetch_loaded_models_sync(&state, &caps, &runtime)?;
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
            id: CommandId { namespace: "lmstudio".into(), action: "unload-model".into(), version: 1 },
            owner: "provider-lmstudio".into(),
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
            let stopped = unload_any_model_sync(&state, &caps, &runtime, &model);
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
            id: CommandId { namespace: "lmstudio".into(), action: "generate".into(), version: 1 },
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
    crate::extensions::provider_common::resolve_system_prompt("provider-lmstudio", cmds, input)
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
fn fetch_model_catalog_sync(
    runtime: &Runtime,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<String>, CoreError> {
    let http = client.clone();
    let base = base_url.to_owned();
    runtime.block_on_compat(async move { fetch_model_catalog(&http, &base).await })
}

fn ping_server_sync(
    runtime: &Runtime,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<(), CoreError> {
    let http = client.clone();
    let base = base_url.to_owned();
    runtime.block_on_compat(async move { ping_server(&http, &base).await })
}

fn unload_all_nulqor_sync(state: &ProviderState, caps: &Capabilities, runtime: &Runtime) {
    let entries: Vec<NulqorLoadedInstance> = state.nulqor_loaded.write().unwrap().drain(..).collect();
    for entry in entries {
        if let Err(e) = unload_tracked_instance_sync(state, caps, runtime, &entry) {
            eprintln!("[provider-lmstudio] unload on disconnect failed: {e}");
        }
    }
}

fn unload_nulqor_model_sync(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
    model: &str,
) -> bool {
    let entry = {
        let mut loaded = state.nulqor_loaded.write().unwrap();
        let pos = loaded.iter().position(|entry| entry.model == model);
        pos.map(|idx| loaded.remove(idx))
    };
    let Some(entry) = entry else {
        return false;
    };
    unload_tracked_instance_sync(state, caps, runtime, &entry)
        .map_err(|e| eprintln!("[provider-lmstudio] unload failed: {e}"))
        .is_ok()
}

fn unload_tracked_instance_sync(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
    entry: &NulqorLoadedInstance,
) -> Result<(), CoreError> {
    let base = state.base_url.read().unwrap().clone();
    let root = server_root(&base);
    caps.check_http_allowed("provider-lmstudio", &root)?;
    let http = state.http_load.clone();
    let instance_id = entry.instance_id.clone();
    runtime.block_on_compat(async move { unload_instance(&http, &root, &instance_id).await })
}

fn nulqor_owns_model(nulqor_loaded: &[NulqorLoadedInstance], model: &str, instance_id: &str) -> bool {
    nulqor_loaded.iter().any(|tracked| {
        tracked.instance_id == instance_id
            || tracked.model == model
            || model_ids_match(&tracked.model, model)
            || model_ids_match(&tracked.instance_id, instance_id)
    })
}

fn track_nulqor_instance(state: &ProviderState, model: &str, instance_id: &str) {
    let mut loaded = state.nulqor_loaded.write().unwrap();
    loaded.retain(|entry| {
        entry.instance_id != instance_id && !model_ids_match(&entry.model, model)
    });
    loaded.push(NulqorLoadedInstance {
        model: model.to_owned(),
        instance_id: instance_id.to_owned(),
    });
}

fn nulqor_loaded_active(state: &ProviderState) -> bool {
    let active = state.active_model.read().unwrap().clone();
    let Some(model) = active else {
        return false;
    };
    let loaded = state.nulqor_loaded.read().unwrap();
    loaded.iter().any(|entry| model_ids_match(&entry.model, &model))
}

fn spin_up_model(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
    model: &str,
) -> Result<bool, CoreError> {
    let base = state.base_url.read().unwrap().clone();
    let root = server_root(&base);
    caps.check_http_allowed("provider-lmstudio", &root)?;

    let http_probe = state.http_probe.clone();
    let http_load = state.http_load.clone();
    let root_clone = root.clone();
    let model_owned = model.to_owned();
    let instance_id = runtime.block_on_compat(async move {
        if let Some(existing) =
            find_existing_instance(&http_probe, &root_clone, &model_owned).await
        {
            return Ok(existing);
        }
        load_model(&http_load, &root_clone, &model_owned).await
    })?;

    track_nulqor_instance(state, model, &instance_id);
    *state.active_model.write().unwrap() = Some(model.to_owned());
    Ok(true)
}

fn server_root(openai_base: &str) -> String {
    openai_base.trim_end_matches('/').trim_end_matches("/v1").to_owned()
}

fn lm_model_id(entry: &serde_json::Value) -> Option<String> {
    if entry["type"].as_str() != Some("llm") {
        return None;
    }
    if let Some(id) = entry["selected_variant"].as_str() {
        return Some(id.to_owned());
    }
    if let Some(variants) = entry["variants"].as_array() {
        if let Some(id) = variants.first().and_then(|v| v.as_str()) {
            return Some(id.to_owned());
        }
    }
    if let Some(instances) = entry["loaded_instances"].as_array() {
        if let Some(id) = instances.first().and_then(|i| i["id"].as_str()) {
            return Some(id.to_owned());
        }
    }
    entry["key"].as_str().map(str::to_owned)
}

fn loaded_lm_model_id(entry: &serde_json::Value) -> Option<String> {
    entry["loaded_instances"]
        .as_array()
        .and_then(|instances| instances.first())
        .and_then(|instance| instance["id"].as_str())
        .map(str::to_owned)
}

fn parse_loaded_from_native(
    native: &serde_json::Value,
    nulqor_loaded: &[NulqorLoadedInstance],
) -> Vec<serde_json::Value> {
    use crate::extensions::provider_common::loaded_entry;

    let mut out = Vec::new();
    for entry in native["models"].as_array().unwrap_or(&vec![]) {
        if entry["type"].as_str() != Some("llm") {
            continue;
        }
        let model_name = lm_model_id(entry).unwrap_or_default();
        let Some(instances) = entry["loaded_instances"].as_array() else {
            continue;
        };
        for inst in instances {
            let Some(instance_id) = inst["id"].as_str() else {
                continue;
            };
            let nulqor_owned = nulqor_owns_model(&nulqor_loaded, &model_name, instance_id);
            out.push(loaded_entry(
                &model_name,
                nulqor_owned,
                Some(instance_id),
                true,
            ));
        }
    }
    out
}

fn fetch_loaded_models_sync(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
) -> Result<Vec<serde_json::Value>, CoreError> {
    let base = state.base_url.read().unwrap().clone();
    let root = server_root(&base);
    caps.check_http_allowed("provider-lmstudio", &root)?;
    let http = state.http_probe.clone();
    let nulqor = state.nulqor_loaded.read().unwrap().clone();
    runtime.block_on_compat(async move {
        let native = fetch_native_models(&http, &root).await?;
        Ok(parse_loaded_from_native(&native, &nulqor))
    })
}

fn unload_any_model_sync(
    state: &ProviderState,
    caps: &Capabilities,
    runtime: &Runtime,
    model: &str,
) -> bool {
    if unload_nulqor_model_sync(state, caps, runtime, model) {
        return true;
    }

    let base = state.base_url.read().unwrap().clone();
    let root = server_root(&base);
    if caps.check_http_allowed("provider-lmstudio", &root).is_err() {
        return false;
    }

    let http_probe = state.http_probe.clone();
    let http_load = state.http_load.clone();
    let model = model.to_owned();
    let root_clone = root.clone();
    runtime.block_on_compat(async move {
        let Ok(native) = fetch_native_models(&http_probe, &root_clone).await else {
            return false;
        };
        for entry in native["models"].as_array().unwrap_or(&vec![]) {
            if lm_model_id(entry).as_deref() != Some(model.as_str()) {
                continue;
            }
            let Some(instance_id) = loaded_lm_model_id(entry) else {
                continue;
            };
            return unload_instance(&http_load, &root_clone, &instance_id)
                .await
                .is_ok();
        }
        false
    })
}

fn dedupe_preserve(mut ids: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    ids
}

fn parse_native_models(json: &serde_json::Value) -> Vec<String> {
    let entries = json["models"].as_array().cloned().unwrap_or_default();
    let mut ids = Vec::new();
    for entry in &entries {
        if let Some(id) = lm_model_id(entry) {
            ids.push(id);
        }
    }
    dedupe_preserve(ids)
}

// ---------------------------------------------------------------------------
// Async helpers
// ---------------------------------------------------------------------------

async fn ping_server(client: &reqwest::Client, base_url: &str) -> Result<(), CoreError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;
    if resp.status().is_success() {
        return Ok(());
    }
    Err(CoreError::Io(format!(
        "LM Studio unreachable at {url} ({})",
        resp.status()
    )))
}

/// Fetch model ids from OpenAI `/v1/models`, falling back to LM Studio `/api/v1/models`.
async fn fetch_model_catalog(client: &reqwest::Client, base_url: &str) -> Result<Vec<String>, CoreError> {
    let openai_ids = fetch_openai_models(client, base_url).await?;
    if !openai_ids.is_empty() {
        return Ok(dedupe_preserve(openai_ids));
    }

    let root = server_root(base_url);
    let native = fetch_native_models(client, &root).await.unwrap_or_default();
    Ok(parse_native_models(&native))
}

async fn find_existing_instance(
    client: &reqwest::Client,
    server_root: &str,
    model: &str,
) -> Option<String> {
    let native = fetch_native_models(client, server_root).await.ok()?;
    for entry in native["models"].as_array()? {
        if lm_model_id(entry).as_deref() != Some(model) {
            continue;
        }
        return loaded_lm_model_id(entry);
    }
    None
}

/// Fetch model ids from `/v1/models`. Never hardcodes an id (decisions/006 §11 #7).
async fn fetch_openai_models(client: &reqwest::Client, base_url: &str) -> Result<Vec<String>, CoreError> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
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

async fn fetch_native_models(client: &reqwest::Client, server_root: &str) -> Result<serde_json::Value, CoreError> {
    let url = format!("{}/api/v1/models", server_root.trim_end_matches('/'));
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;

    resp.json()
        .await
        .map_err(|e| CoreError::Io(format!("native models JSON: {e}")))
}

async fn load_model(client: &reqwest::Client, server_root: &str, model: &str) -> Result<String, CoreError> {
    let url = format!("{}/api/v1/models/load", server_root.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "model": model }))
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("POST {url}: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(CoreError::Io(format!(
            "load model '{model}' failed ({status}): {body}"
        )));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| CoreError::Io(format!("load model JSON: {e}")))?;
    Ok(json["instance_id"]
        .as_str()
        .unwrap_or(model)
        .to_owned())
}

async fn unload_instance(
    client: &reqwest::Client,
    server_root: &str,
    instance_id: &str,
) -> Result<(), CoreError> {
    let url = format!("{}/api/v1/models/unload", server_root.trim_end_matches('/'));
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "instance_id": instance_id }))
        .send()
        .await
        .map_err(|e| CoreError::Io(format!("POST {url}: {e}")))?;

    if resp.status().is_success() {
        return Ok(());
    }

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    Err(CoreError::Io(format!(
        "unload instance '{instance_id}' failed ({status}): {body}"
    )))
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
                "error": "no model selected — connect to LM Studio and pick a model",
            }),
        );
        return;
    }

    // Keep UI / HTTP `active` in sync when callers pass an explicit model id.
    if let Ok(mut active) = state.active_model.write() {
        *active = Some(model.clone());
    }

    let body = serde_json::json!({
        "model": model,
        "messages": input["messages"],
        "stream": true,
        "temperature": input["temperature"].as_f64().unwrap_or(0.7),
        "max_tokens": input["max_tokens"].as_i64().unwrap_or(2048),
    });

    let resp = match state
        .http_generate
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
            } else if let Some(r) = json["choices"][0]["delta"]["reasoning"].as_str() {
                reasoning.push_str(r);
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
        assert!(cmds.iter().any(|c| c == "lmstudio:connect@1"), "connect not registered: {cmds:?}");
        assert!(cmds.iter().any(|c| c == "lmstudio:select-model@1"), "select-model not registered: {cmds:?}");
        assert!(cmds.iter().any(|c| c == "lmstudio:models@1"), "models not registered: {cmds:?}");
        assert!(cmds.iter().any(|c| c == "lmstudio:generate@1"), "generate not registered: {cmds:?}");
    }

    #[test]
    fn models_returns_cache_without_refresh() {
        let ctx = make_context();
        let ext = LmStudioProvider::new(make_manifest());
        ext.activate(&ctx).unwrap();

        let result = ctx.commands.invoke(
            "test",
            &crate::types::CommandId::parse("lmstudio:models@1").unwrap(),
            serde_json::json!({}),
        );
        assert!(result.is_ok());
        let json = result.unwrap();
        assert_eq!(json["models"].as_array().unwrap().len(), 0);
        assert!(json["active"].is_null());
        assert_eq!(json["connected"], false);
        assert_eq!(json["nulqor_loaded_active"], false);
    }

    #[test]
    fn parse_native_models_extracts_llm_variants() {
        let json = serde_json::json!({
            "models": [
                {
                    "type": "llm",
                    "key": "qwen2.5-7b-instruct",
                    "selected_variant": "qwen2.5-7b-instruct@q4_k_m",
                    "loaded_instances": []
                },
                {
                    "type": "embedding",
                    "key": "nomic-embed"
                }
            ]
        });
        let ids = parse_native_models(&json);
        assert_eq!(ids, vec!["qwen2.5-7b-instruct@q4_k_m".to_owned()]);
    }

    #[test]
    fn nulqor_loaded_active_tracks_only_owned_models() {
        let state = ProviderState::new();
        *state.active_model.write().unwrap() = Some("model-a".into());
        *state.nulqor_loaded.write().unwrap() = vec![NulqorLoadedInstance {
            model: "model-a".into(),
            instance_id: "inst-a".into(),
        }];
        assert!(nulqor_loaded_active(&state));
        *state.active_model.write().unwrap() = Some("external-model".into());
        assert!(!nulqor_loaded_active(&state));
    }

    #[test]
    fn disconnect_returns_ok() {
        let ctx = make_context();
        let ext = LmStudioProvider::new(make_manifest());
        ext.activate(&ctx).unwrap();

        let result = ctx.commands.invoke(
            "test",
            &crate::types::CommandId::parse("lmstudio:disconnect@1").unwrap(),
            serde_json::json!({}),
        );
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["ok"], true);
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
            &crate::types::CommandId::parse("lmstudio:connect@1").unwrap(),
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
            &crate::types::CommandId::parse("lmstudio:connect@1").unwrap(),
            serde_json::json!({ "url": "http://evil.example.com", "model": "test-model" }),
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
            &crate::types::CommandId::parse("lmstudio:generate@1").unwrap(),
            serde_json::json!({ "messages": [] }),
        );
        assert!(result.is_ok(), "generate should return Ok immediately: {result:?}");
        let val = result.unwrap();
        assert!(val["stream_id"].is_string(), "should have stream_id: {val}");
    }
}
