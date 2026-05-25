//! Transcript / session extension — Phase 2.2 (BUILD_PLAN §2.2, decisions/006 §5).
//!
//! The one shared in-memory session. Messages carry the schema in decisions/006 §5.
//! Listens for `provider:stream-done@1` to auto-record assistant turns.
//! Emits `transcript:message-added@1` after every append.
//!
//! Thread-safety: the session is wrapped in `Arc<std::sync::RwLock<_>>`. All
//! command handlers and event subscribers are synchronous closures so no async
//! lock primitives are required.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::events::EventBus;
use crate::types::{
    CommandDecl, CommandId, EventId, EventPattern, ExtensionManifest, NamespacedEvent, Permission,
};

// ---------------------------------------------------------------------------
// Message schema (decisions/006 §5)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub latency_ms: u64,
    pub tokens: u64,
    pub driver: String,
    pub participant_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

impl Message {
    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

// ---------------------------------------------------------------------------
// Session state
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Session {
    pub messages: Vec<Message>,
    pub active_agent: String,
}

impl Session {
    fn transcript_hash(&self) -> String {
        let content_len: usize = self.messages.iter().map(|m| m.content.len()).sum();
        format!("{}-{}", self.messages.len(), content_len)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "messages": self.messages.iter().map(|m| m.to_json()).collect::<Vec<_>>(),
            "transcript_hash": self.transcript_hash(),
        })
    }
}

pub type SharedSession = Arc<RwLock<Session>>;

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

pub struct TranscriptExtension {
    manifest: ExtensionManifest,
    pub session: SharedSession,
}

impl TranscriptExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self {
            manifest,
            session: Arc::new(RwLock::new(Session::default())),
        }
    }
}

impl Extension for TranscriptExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let session = self.session.clone();
        let bus = ctx.bus.clone();

        register_get(session.clone(), &ctx.commands)?;
        register_add_user_message(session.clone(), bus.clone(), &ctx.commands)?;
        register_clear(session.clone(), bus.clone(), &ctx.commands)?;
        register_set_active_agent(session.clone(), &ctx.commands)?;
        subscribe_stream_done(session, bus, ctx);

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn register_get(
    session: SharedSession,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "transcript".into(), action: "get".into(), version: 1 },
            owner: "transcript".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "messages": "array", "transcript_hash": "string" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| Ok(session.read().unwrap().to_json())),
    )
}

fn register_add_user_message(
    session: SharedSession,
    bus: Arc<EventBus>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "transcript".into(),
                action: "add-user-message".into(),
                version: 1,
            },
            owner: "transcript".into(),
            input_schema: r#"{ "content": "string", "observer_name": "string" }"#.into(),
            output_schema: r#"{ "id": "string" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into(), "service".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let content = input["content"]
                .as_str()
                .ok_or_else(|| CoreError::Io("add-user-message: 'content' required".into()))?
                .to_owned();
            let observer_name = input["observer_name"]
                .as_str()
                .ok_or_else(|| CoreError::Io("add-user-message: 'observer_name' required".into()))?
                .to_owned();
            let driver = input["driver"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| observer_name.clone());
            let agent = input["agent"].as_str().map(str::to_owned);

            let msg = Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: "user".into(),
                content: content.clone(),
                timestamp: Utc::now(),
                model: None,
                latency_ms: 0,
                tokens: 0,
                driver: driver.clone(),
                participant_name: observer_name.clone(),
                reasoning: None,
                agent: agent.clone(),
            };
            let msg_id = msg.id.clone();
            let msg_json = msg.to_json();

            session.write().unwrap().messages.push(msg);

            let _ = bus.publish(NamespacedEvent {
                id: EventId {
                    namespace: "transcript".into(),
                    name: "message-added".into(),
                    version: 1,
                },
                payload: serde_json::json!({ "message": msg_json }),
            });

            Ok(serde_json::json!({ "id": msg_id }))
        }),
    )
}

fn register_clear(
    session: SharedSession,
    bus: Arc<EventBus>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "transcript".into(),
                action: "clear".into(),
                version: 1,
            },
            owner: "transcript".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "cleared": "number" }"#.into(),
            callable_by: vec!["panel".into()],
            permission: Permission::Destructive,
        },
        Arc::new(move |_| {
            let count = {
                let mut s = session.write().unwrap();
                let n = s.messages.len();
                s.messages.clear();
                n
            };

            let _ = bus.publish(NamespacedEvent {
                id: EventId {
                    namespace: "transcript".into(),
                    name: "cleared".into(),
                    version: 1,
                },
                payload: serde_json::json!({ "cleared": count }),
            });

            Ok(serde_json::json!({ "cleared": count }))
        }),
    )
}

fn register_set_active_agent(
    session: SharedSession,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "transcript".into(),
                action: "set-active-agent".into(),
                version: 1,
            },
            owner: "transcript".into(),
            input_schema: r#"{ "agent": "string" }"#.into(),
            output_schema: r#"{ "ok": "boolean" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Write,
        },
        Arc::new(move |input| {
            let agent = input["agent"]
                .as_str()
                .ok_or_else(|| CoreError::Io("set-active-agent: 'agent' required".into()))?
                .to_owned();
            session.write().unwrap().active_agent = agent;
            Ok(serde_json::json!({ "ok": true }))
        }),
    )
}

// ---------------------------------------------------------------------------
// Event subscription: provider:stream-done@1 → record assistant turn
// ---------------------------------------------------------------------------

fn subscribe_stream_done(session: SharedSession, bus: Arc<EventBus>, ctx: &CoreContext) {
    ctx.bus.subscribe(
        EventPattern::exact("provider", "stream-done", 1),
        Arc::new(move |ev: &NamespacedEvent| {
            let p = &ev.payload;
            let content = p["content"].as_str().unwrap_or("").to_owned();
            let model = p["model"].as_str().map(str::to_owned);
            let tokens = p["tokens"].as_u64().unwrap_or(0);
            let reasoning = p["reasoning"].as_str().map(str::to_owned);

            let msg = Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: "assistant".into(),
                content: content.clone(),
                timestamp: Utc::now(),
                model: model.clone(),
                latency_ms: 0,
                tokens,
                driver: "assistant".into(),
                participant_name: model.clone().unwrap_or_else(|| "Model".into()),
                reasoning,
                agent: None,
            };
            let msg_json = msg.to_json();

            session.write().unwrap().messages.push(msg);

            let _ = bus.publish(NamespacedEvent {
                id: EventId {
                    namespace: "transcript".into(),
                    name: "message-added".into(),
                    version: 1,
                },
                payload: serde_json::json!({ "message": msg_json }),
            });
        }),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ExtensionKind;

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

    fn make_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: "transcript".into(),
            version: semver::Version::parse("0.1.0").unwrap(),
            kind: ExtensionKind::Service,
            api_version: "v1".into(),
            schema_version: semver::Version::parse("1.0.0").unwrap(),
            min_core: semver::Version::parse("0.1.0").unwrap(),
            requires: vec!["provider-lmstudio".into()],
            optional: vec![],
            provides: vec![],
            commands: vec![],
            publishes: vec![],
            subscribes: vec![],
            fs_scopes: vec![],
            http_hosts: vec![],
        }
    }

    #[test]
    fn transcript_registers_four_commands() {
        let ctx = make_context();
        let ext = TranscriptExtension::new(make_manifest());
        ext.activate(&ctx).expect("activate");

        let cmds = ctx.commands.list_commands();
        assert!(cmds.iter().any(|c| c == "transcript:get@1"));
        assert!(cmds.iter().any(|c| c == "transcript:add-user-message@1"));
        assert!(cmds.iter().any(|c| c == "transcript:clear@1"));
        assert!(cmds.iter().any(|c| c == "transcript:set-active-agent@1"));
    }

    #[test]
    fn add_user_message_appends_and_emits() {
        let ctx = make_context();
        let ext = TranscriptExtension::new(make_manifest());

        let received = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let r = received.clone();
        ctx.bus.subscribe(
            EventPattern::exact("transcript", "message-added", 1),
            Arc::new(move |_| {
                r.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }),
        );

        ext.activate(&ctx).unwrap();

        let result = ctx.commands.invoke(
            "test",
            &CommandId::parse("transcript:add-user-message@1").unwrap(),
            serde_json::json!({ "content": "hello", "observer_name": "test-user" }),
        );
        assert!(result.is_ok(), "add-user-message failed: {result:?}");
        assert!(result.unwrap()["id"].is_string());

        assert_eq!(received.load(std::sync::atomic::Ordering::SeqCst), 1);

        let transcript = ctx
            .commands
            .invoke(
                "test",
                &CommandId::parse("transcript:get@1").unwrap(),
                serde_json::json!({}),
            )
            .unwrap();
        assert_eq!(transcript["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn stream_done_event_appends_assistant_turn() {
        let ctx = make_context();
        let ext = TranscriptExtension::new(make_manifest());
        ext.activate(&ctx).unwrap();

        ctx.bus
            .publish(NamespacedEvent {
                id: EventId {
                    namespace: "provider".into(),
                    name: "stream-done".into(),
                    version: 1,
                },
                payload: serde_json::json!({
                    "stream_id": "test-123",
                    "content": "I am the model reply",
                    "tokens": 42,
                    "model": "test-model",
                }),
            })
            .unwrap();

        let transcript = ctx
            .commands
            .invoke(
                "test",
                &CommandId::parse("transcript:get@1").unwrap(),
                serde_json::json!({}),
            )
            .unwrap();
        let msgs = transcript["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1, "should have one assistant message");
        assert_eq!(msgs[0]["role"], "assistant");
        assert_eq!(msgs[0]["content"], "I am the model reply");
        assert_eq!(msgs[0]["tokens"], 42);
    }
}
