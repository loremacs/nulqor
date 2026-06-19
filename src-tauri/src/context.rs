//! CoreContext, Extension trait, ConfigStore, and AppState.
//!
//! CoreContext is the ONE handle every extension receives in `activate()`.
//! It is the entire surface an extension may touch — nothing else is reachable.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::capability::{CapabilityRegistry, Capabilities};
use crate::commands::CommandRegistry;
use crate::error::CoreError;
use crate::events::EventBus;
use crate::permission::PermissionGate;
use crate::runtime::Runtime;
use crate::types::{CommandDecl, ExtensionManifest, NamespacedEvent};
use crate::version::VersionManager;

// ---------------------------------------------------------------------------
// Extension trait — the only contract between the core and extension code
// ---------------------------------------------------------------------------

/// Every extension implements this. The core calls these three methods — nothing else.
/// Keep it tiny: the core hosts; it does not understand what the extension does.
pub trait Extension: Send + Sync {
    fn manifest(&self) -> &ExtensionManifest;
    /// Register commands, events, capabilities, and subscriptions via `ctx`. Called once on load.
    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError>;
    /// Optional clean shutdown. Called when the extension is deactivated.
    fn deactivate(&self, _ctx: &CoreContext) -> Result<(), CoreError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ConfigStore — per-extension key/value config
// ---------------------------------------------------------------------------

pub trait ConfigStore: Send + Sync {
    fn get(&self, ext_id: &str, key: &str) -> Result<serde_json::Value, CoreError>;
    fn set(&self, ext_id: &str, key: &str, value: serde_json::Value) -> Result<(), CoreError>;
}

pub struct InMemoryConfigStore {
    data: RwLock<HashMap<String, serde_json::Value>>,
}

impl InMemoryConfigStore {
    pub fn new() -> Self {
        Self { data: RwLock::new(HashMap::new()) }
    }

    fn config_key(ext_id: &str, key: &str) -> String {
        format!("{ext_id}::{key}")
    }
}

impl ConfigStore for InMemoryConfigStore {
    fn get(&self, ext_id: &str, key: &str) -> Result<serde_json::Value, CoreError> {
        let k = Self::config_key(ext_id, key);
        Ok(self
            .data
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .get(&k)
            .cloned()
            .unwrap_or(serde_json::Value::Null))
    }

    fn set(&self, ext_id: &str, key: &str, value: serde_json::Value) -> Result<(), CoreError> {
        let k = Self::config_key(ext_id, key);
        self.data.write().unwrap_or_else(|p| p.into_inner()).insert(k, value);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CoreContext — the complete interface an extension receives
// ---------------------------------------------------------------------------

/// Everything an extension may access. All fields are `Arc` over `Send + Sync` impls
/// so this struct itself is `Send + Sync`, making it safe to hold in Tauri state.
#[derive(Clone)]
pub struct CoreContext {
    pub bus: Arc<EventBus>,
    pub commands: Arc<CommandRegistry>,
    pub versions: Arc<VersionManager>,
    pub permissions: Arc<PermissionGate>,
    pub caps: Arc<Capabilities>,
    pub capability_registry: Arc<CapabilityRegistry>,
    pub runtime: Arc<Runtime>,
    pub config: Arc<dyn ConfigStore>,
}

impl CoreContext {
    /// Convenience: publish a named event.
    pub fn emit(&self, namespace: &str, name: &str, version: u32, payload: serde_json::Value) {
        use crate::types::EventId;
        let ev = NamespacedEvent {
            id: EventId { namespace: namespace.into(), name: name.into(), version },
            payload,
        };
        if let Err(e) = self.bus.publish(ev) {
            eprintln!("[CORE] emit error: {e}");
        }
    }

    /// Convenience: register a read command with a simple closure.
    pub fn register_read_command(
        &self,
        namespace: &str,
        action: &str,
        version: u32,
        owner: &str,
        handler: impl Fn(serde_json::Value) -> Result<serde_json::Value, CoreError>
            + Send
            + Sync
            + 'static,
    ) -> Result<(), CoreError> {
        use crate::types::{CommandId, Permission};
        self.commands.register(
            CommandDecl {
                id: CommandId { namespace: namespace.into(), action: action.into(), version },
                owner: owner.into(),
                input_schema: "{}".into(),
                output_schema: "{}".into(),
                callable_by: vec!["panel".into(), "agent".into()],
                permission: Permission::Read,
            },
            std::sync::Arc::new(handler),
        )
    }
}

// ---------------------------------------------------------------------------
// AppState — Tauri managed-state wrapper around CoreContext
// ---------------------------------------------------------------------------

pub struct AppState {
    pub core: CoreContext,
}

impl AppState {
    pub fn new(core: CoreContext) -> Self {
        Self { core }
    }
}
