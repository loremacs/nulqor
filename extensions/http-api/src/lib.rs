//! HTTP + WebSocket API extension — Phase 2.3 (BUILD_PLAN §2.3, decisions/006 §1–3).
//!
//! Starts an axum HTTP server on port 8787 (override with the `NULQOR_PORT` env var).
//! Note: 8080 is intentionally avoided because it collides with the conventional
//! llama.cpp model-server default (see `provider-llamacpp`).
//! Implements the exact endpoint surface from decisions/006 §1.
//! WebSocket paths forward transcript events to connected clients.
//! Observer/catch-up protocol per decisions/006 §3.
//!
//! Lifecycle: the server starts in a detached Tokio task via `runtime.spawn_task`.
//! Shutdown is graceful when the Tauri app exits (the tokio runtime is dropped).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State as AxumState};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;

use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::types::{
    CommandDecl, CommandId, EventPattern, ExtensionManifest, NamespacedEvent, Permission,
};

// ---------------------------------------------------------------------------
// Observer registry (decisions/006 §3)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Observer {
    name: String,
    last_ack_seq: u64,
}

#[derive(Clone, Debug, Serialize)]
struct CatchUpEvent {
    seq: u64,
    event: serde_json::Value,
}

struct ObserverRegistry {
    observers: HashMap<String, Observer>,
    log: Vec<CatchUpEvent>, // append-only; seq = index+1
}

impl ObserverRegistry {
    fn new() -> Self {
        Self { observers: HashMap::new(), log: Vec::new() }
    }

    /// Register or return existing (duplicate name is idempotent — decisions/006 §3).
    fn register(&mut self, name: &str) -> &Observer {
        let lower = name.to_lowercase();
        if !self.observers.contains_key(&lower) {
            let generated = if name.is_empty() {
                format!("agent-{}", &uuid::Uuid::new_v4().to_string()[..6])
            } else {
                name.to_owned()
            };
            self.observers.insert(lower.clone(), Observer { name: generated, last_ack_seq: 0 });
        }
        self.observers.get(&lower).unwrap()
    }

    /// Append a message_added event to the catch-up log (stream events excluded — §3).
    fn append_message_added(&mut self, message_json: serde_json::Value) {
        let seq = self.log.len() as u64 + 1;
        self.log.push(CatchUpEvent {
            seq,
            event: serde_json::json!({ "type": "message_added", "message": message_json }),
        });
    }

    /// Return events with seq > last_ack_seq. Optionally advance ack pointer.
    fn catch_up(&mut self, name: &str, auto_ack: bool) -> Option<Vec<CatchUpEvent>> {
        let lower = name.to_lowercase();
        let obs = self.observers.get_mut(&lower)?;
        let from = obs.last_ack_seq;
        let events: Vec<CatchUpEvent> =
            self.log.iter().filter(|e| e.seq > from).cloned().collect();
        if auto_ack {
            if let Some(last) = events.last() {
                obs.last_ack_seq = last.seq;
            }
        }
        Some(events)
    }

    /// Advance ack to current head without returning events.
    fn ack(&mut self, name: &str) {
        let lower = name.to_lowercase();
        let head = self.log.len() as u64;
        if let Some(obs) = self.observers.get_mut(&lower) {
            obs.last_ack_seq = head;
        }
    }
}

// ---------------------------------------------------------------------------
// Shared API state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ApiState {
    commands: Arc<crate::commands::CommandRegistry>,
    observers: Arc<RwLock<ObserverRegistry>>,
    /// Broadcast channel for WebSocket fan-out.
    ws_tx: broadcast::Sender<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

const DEFAULT_PORT: u16 = 8787;

/// Resolve the listen port: `NULQOR_PORT` env override, else `DEFAULT_PORT`.
fn resolve_port() -> u16 {
    std::env::var("NULQOR_PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(DEFAULT_PORT)
}

pub struct HttpApiExtension {
    manifest: ExtensionManifest,
}

impl HttpApiExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }
}

impl Extension for HttpApiExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let (ws_tx, _) = broadcast::channel::<serde_json::Value>(256);
        let observers = Arc::new(RwLock::new(ObserverRegistry::new()));

        let api_state = ApiState {
            commands: ctx.commands.clone(),
            observers: observers.clone(),
            ws_tx: ws_tx.clone(),
        };

        // Wire event subscriptions → observer log + WebSocket fan-out
        wire_transcript_events(ctx.bus.clone(), observers.clone(), ws_tx.clone());

        // Register http-api:status@1
        {
            let port = resolve_port();
            let running = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let running_clone = running.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "http-api".into(),
                        action: "status".into(),
                        version: 1,
                    },
                    owner: "http-api".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "running": "boolean", "port": "number" }"#.into(),
                    callable_by: vec!["panel".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    Ok(serde_json::json!({
                        "running": running_clone.load(std::sync::atomic::Ordering::SeqCst),
                        "port": port,
                    }))
                }),
            )?;

            // Start the server in a background task
            ctx.runtime.spawn_task(Duration::from_secs(u64::MAX), async move {
                let app = build_router(api_state);
                let addr = SocketAddr::from(([127, 0, 0, 1], port));
                eprintln!("[http-api] listening on http://{addr}");
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[http-api] bind error: {e}");
                        return;
                    }
                };
                running.store(true, std::sync::atomic::Ordering::SeqCst);
                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("[http-api] server error: {e}");
                }
            });
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Event wiring
// ---------------------------------------------------------------------------

fn wire_transcript_events(
    bus: Arc<EventBus>,
    observers: Arc<RwLock<ObserverRegistry>>,
    ws_tx: broadcast::Sender<serde_json::Value>,
) {
    // transcript:message-added@1 → observer log + WebSocket
    let obs = observers.clone();
    let tx = ws_tx.clone();
    bus.subscribe(
        EventPattern::exact("transcript", "message-added", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            let msg = ev.payload["message"].clone();
            let ws_event = serde_json::json!({ "type": "message_added", "message": msg.clone() });
            let _ = tx.send(ws_event);
            obs.write().unwrap_or_else(|p| p.into_inner()).append_message_added(msg);
        }),
    );

    // provider stream events → WebSocket only (not in catch-up log — §3)
    for (event_name, ws_type) in [
        ("stream-start", "stream_start"),
        ("stream-delta", "stream_delta"),
        ("stream-done", "stream_done"),
    ] {
        let tx = ws_tx.clone();
        let ws_type = ws_type.to_owned();
        bus.subscribe(
            EventPattern::exact("provider", event_name, 1),
            Arc::new(move |ev: &NamespacedEvent| {
                let mut payload = ev.payload.clone();
                payload["type"] = serde_json::json!(ws_type);
                let _ = tx.send(payload);
            }),
        );
    }
}

// ---------------------------------------------------------------------------
// Axum router
// ---------------------------------------------------------------------------

fn build_router(state: ApiState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health))
        .route("/models", get(list_models))
        .route("/connect", post(connect))
        .route("/select-model", post(select_model))
        .route("/stop-model", post(stop_model))
        .route("/transcript", get(get_transcript))
        .route("/message", post(send_message))
        .route("/observers/register", post(register_observer))
        .route("/observers", get(list_observers))
        .route("/observers/catch-up", get(catch_up))
        .route("/observers/ack", post(ack_observer))
        .route("/ws/transcript", get(ws_transcript_handler))
        .route("/ws/chat", get(ws_transcript_handler))
        .layer(cors)
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn list_models(
    AxumState(s): AxumState<ApiState>,
    Query(query): Query<ModelsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let refresh = query.refresh.unwrap_or(false);
    let mut input = serde_json::json!({ "refresh": refresh });
    if let Some(url) = query.url {
        input["url"] = serde_json::Value::String(url);
    }
    let result = s.commands.invoke(
        "http-api",
        &CommandId::parse("provider:models@1").unwrap(),
        input,
    )?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct ModelsQuery {
    refresh: Option<bool>,
    url: Option<String>,
}

#[derive(Deserialize)]
struct ConnectBody {
    url: String,
    model: String,
}

async fn connect(
    AxumState(s): AxumState<ApiState>,
    Json(body): Json<ConnectBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = s.commands.invoke(
        "http-api",
        &CommandId::parse("provider:connect@1").unwrap(),
        serde_json::json!({ "url": body.url, "model": body.model }),
    )?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct SelectModelBody {
    model: String,
}

async fn select_model(
    AxumState(s): AxumState<ApiState>,
    Json(body): Json<SelectModelBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let result = s.commands.invoke(
        "http-api",
        &CommandId::parse("provider:select-model@1").unwrap(),
        serde_json::json!({ "model": body.model }),
    )?;
    Ok(Json(result))
}

async fn stop_model(AxumState(s): AxumState<ApiState>) -> Result<Json<serde_json::Value>, ApiError> {
    let result = s.commands.invoke(
        "http-api",
        &CommandId::parse("provider:stop-model@1").unwrap(),
        serde_json::json!({}),
    )?;
    Ok(Json(result))
}

async fn get_transcript(AxumState(s): AxumState<ApiState>) -> Result<Json<serde_json::Value>, ApiError> {
    let result = s.commands.invoke("http-api", &CommandId::parse("transcript:get@1").unwrap(), serde_json::json!({}))?;
    Ok(Json(result))
}

#[derive(Deserialize)]
struct SendMessageBody {
    message: String,
    observer_name: String,
    model: Option<String>,
    agent: Option<String>,
}

async fn send_message(
    AxumState(s): AxumState<ApiState>,
    Json(body): Json<SendMessageBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    // Verify observer is registered (decisions/006 §3 — unregistered → 400)
    {
        let obs = s.observers.read().unwrap();
        if !obs.observers.contains_key(&body.observer_name.to_lowercase()) {
            return Err(ApiError::bad_request(format!(
                "observer '{}' not registered",
                body.observer_name
            )));
        }
    }

    // Add user turn
    s.commands.invoke(
        "http-api",
        &CommandId::parse("transcript:add-user-message@1").unwrap(),
        serde_json::json!({
            "content": body.message,
            "observer_name": body.observer_name,
            "agent": body.agent,
        }),
    )?;

    // Fetch transcript for generation
    let transcript = s.commands.invoke(
        "http-api",
        &CommandId::parse("transcript:get@1").unwrap(),
        serde_json::json!({}),
    )?;

    // Start generation (returns stream_id immediately)
    let gen_result = s.commands.invoke(
        "http-api",
        &CommandId::parse("provider:generate@1").unwrap(),
        serde_json::json!({
            "messages": transcript["messages"],
            "model": body.model,
            "agent": body.agent,
        }),
    )?;

    Ok(Json(gen_result))
}

#[derive(Deserialize)]
struct RegisterObserverBody {
    name: Option<String>,
}

async fn register_observer(
    AxumState(s): AxumState<ApiState>,
    Json(body): Json<RegisterObserverBody>,
) -> Json<serde_json::Value> {
    let name = body.name.unwrap_or_default();
    let (out_name, last_ack_seq, pending) = {
        let mut obs = s.observers.write().unwrap();
        let o = obs.register(&name).clone();
        let pending = (obs.log.len() as u64).saturating_sub(o.last_ack_seq);
        (o.name.clone(), o.last_ack_seq, pending)
    };
    Json(serde_json::json!({
        "name": out_name,
        "last_ack_seq": last_ack_seq,
        "pending_count": pending,
    }))
}

async fn list_observers(AxumState(s): AxumState<ApiState>) -> Json<serde_json::Value> {
    let list = {
        let obs = s.observers.read().unwrap();
        obs.observers
            .values()
            .map(|o| {
                let pending = (obs.log.len() as u64).saturating_sub(o.last_ack_seq);
                serde_json::json!({ "name": o.name, "last_ack_seq": o.last_ack_seq, "pending_count": pending })
            })
            .collect::<Vec<_>>()
    };
    Json(serde_json::json!({ "observers": list }))
}

#[derive(Deserialize)]
struct CatchUpParams {
    observer: String,
    auto_ack: Option<bool>,
}

async fn catch_up(
    AxumState(s): AxumState<ApiState>,
    Query(params): Query<CatchUpParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let auto_ack = params.auto_ack.unwrap_or(false);
    let events = {
        let mut obs = s.observers.write().unwrap();
        obs.catch_up(&params.observer, auto_ack)
            .ok_or_else(|| ApiError::bad_request(format!("observer '{}' not found", params.observer)))?
    };
    Ok(Json(serde_json::json!({ "events": events })))
}

#[derive(Deserialize)]
struct AckBody {
    name: String,
}

async fn ack_observer(
    AxumState(s): AxumState<ApiState>,
    Json(body): Json<AckBody>,
) -> Json<serde_json::Value> {
    s.observers.write().unwrap().ack(&body.name);
    Json(serde_json::json!({ "ok": true }))
}

async fn ws_transcript_handler(
    ws: WebSocketUpgrade,
    AxumState(s): AxumState<ApiState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_session(socket, s))
}

async fn ws_session(mut socket: WebSocket, state: ApiState) {
    // Send current snapshot on connect
    if let Ok(transcript) = state.commands.invoke(
        "ws",
        &CommandId::parse("transcript:get@1").unwrap(),
        serde_json::json!({}),
    ) {
        let snap = serde_json::json!({
            "type": "transcript_snapshot",
            "messages": transcript["messages"],
        });
        let _ = socket.send(WsMessage::Text(snap.to_string().into())).await;
    }

    let mut rx = state.ws_tx.subscribe();
    loop {
        tokio::select! {
            ev = rx.recv() => {
                match ev {
                    Ok(msg) => {
                        if socket.send(WsMessage::Text(msg.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                if msg.is_none() { break; }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Error helper
// ---------------------------------------------------------------------------

struct ApiError(StatusCode, String);

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self(StatusCode::BAD_REQUEST, msg.into())
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn make_context() -> CoreContext {
        use crate::{
            capability::{CapabilityRegistry, Capabilities},
            commands::CommandRegistry,
            context::InMemoryConfigStore,
            events::EventBus,
            permission::PermissionGate,
            runtime::Runtime,
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

    fn make_transcript_manifest() -> ExtensionManifest {
        use crate::types::ExtensionKind;
        ExtensionManifest {
            id: "transcript".into(),
            version: semver::Version::parse("0.1.0").unwrap(),
            kind: ExtensionKind::Service,
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
            http_hosts: vec![],
        }
    }

    fn setup_test_router(ctx: &CoreContext) -> Router {
        // Activate transcript extension so transcript:get@1 etc. exist
        let t_ext = crate::extensions::transcript::TranscriptExtension::new(make_transcript_manifest());
        t_ext.activate(ctx).unwrap();

        let (ws_tx, _) = broadcast::channel(16);
        let api_state = ApiState {
            commands: ctx.commands.clone(),
            observers: Arc::new(RwLock::new(ObserverRegistry::new())),
            ws_tx,
        };
        build_router(api_state)
    }

    #[test]
    fn default_port_avoids_model_server_collision() {
        // 8080 is the conventional llama.cpp model-server port; the API must not share it.
        assert_ne!(DEFAULT_PORT, 8080, "API default port must not collide with model servers");
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let ctx = make_context();
        let app = setup_test_router(&ctx);
        let resp = app
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["ok"], true);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn transcript_returns_empty_initially() {
        let ctx = make_context();
        let app = setup_test_router(&ctx);
        let resp = app
            .oneshot(Request::get("/transcript").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["messages"].as_array().unwrap().len(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn register_observer_returns_name() {
        let ctx = make_context();
        let app = setup_test_router(&ctx);
        let resp = app
            .oneshot(
                Request::post("/observers/register")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"cursor-agent"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["name"], "cursor-agent");
        assert_eq!(json["last_ack_seq"], 0);
    }

    #[test]
    fn duplicate_observer_name_is_idempotent() {
        let ctx = make_context();
        let (ws_tx, _) = broadcast::channel(16);
        let api_state = ApiState {
            commands: ctx.commands.clone(),
            observers: Arc::new(RwLock::new(ObserverRegistry::new())),
            ws_tx,
        };
        let t_ext = crate::extensions::transcript::TranscriptExtension::new(make_transcript_manifest());
        t_ext.activate(&ctx).unwrap();

        // Register twice with same name
        api_state.observers.write().unwrap().register("my-agent");
        let o2 = api_state.observers.write().unwrap().register("my-agent").clone();
        assert_eq!(o2.name, "my-agent");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn send_message_requires_registered_observer() {
        let ctx = make_context();
        let app = setup_test_router(&ctx);
        let resp = app
            .oneshot(
                Request::post("/message")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"message":"hi","observer_name":"ghost"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 400, "unregistered observer must return 400");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn catch_up_returns_empty_for_new_observer() {
        let ctx = make_context();
        let app = setup_test_router(&ctx);

        // Register observer
        let _ = app
            .clone()
            .oneshot(
                Request::post("/observers/register")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"name":"test-obs"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let resp = app
            .oneshot(
                Request::get("/observers/catch-up?observer=test-obs&auto_ack=false")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["events"].as_array().unwrap().len(), 0);
    }
}
