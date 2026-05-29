//! Provider router — forwards public `provider:*@1` commands to the active backend.
//!
//! Backends register under their instance namespace (`lmstudio:*`, `ollama:*`, …).
//! The active instance comes from `nulqor.toml` → `active_provider`.

use std::sync::{Arc, RwLock};

use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

#[derive(Clone, Debug)]
pub struct ProviderMeta {
    pub id: &'static str,
    pub label: &'static str,
    pub default_url: &'static str,
    pub extension_id: &'static str,
    pub vram_hint: &'static str,
}

pub const PROVIDERS: &[ProviderMeta] = &[
    ProviderMeta {
        id: "lmstudio",
        label: "LM Studio",
        default_url: "http://localhost:1234",
        extension_id: "provider-lmstudio",
        vram_hint: "7–8B Q4/Q5; one model at a time",
    },
    ProviderMeta {
        id: "ollama",
        label: "Ollama",
        default_url: "http://localhost:11434",
        extension_id: "provider-ollama",
        vram_hint: "llama3.2, phi3, gemma2:2b, qwen2.5:7b-instruct-q4_K_M",
    },
    ProviderMeta {
        id: "llamacpp",
        label: "llama.cpp server",
        default_url: "http://localhost:8080",
        extension_id: "provider-llamacpp",
        vram_hint: "7–8B GGUF Q4_K_M; start server with one model",
    },
];

const ACTIONS: &[&str] = &[
    "connect",
    "disconnect",
    "select-model",
    "stop-model",
    "unload-model",
    "models",
    "loaded-models",
    "generate",
];

pub struct ProviderRouter {
    manifest: ExtensionManifest,
    active: Arc<RwLock<String>>,
}

impl ProviderRouter {
    pub fn new(manifest: ExtensionManifest, active_instance: String) -> Self {
        Self {
            manifest,
            active: Arc::new(RwLock::new(active_instance)),
        }
    }

    fn backend_key(active: &str, action: &str) -> String {
        format!("{active}:{action}@1")
    }

    fn resolve_active(&self) -> Result<String, CoreError> {
        let active = self.active.read().unwrap().clone();
        if PROVIDERS.iter().any(|p| p.id == active) {
            Ok(active)
        } else {
            Err(CoreError::Io(format!(
                "unknown active_provider '{active}' — set active_provider in nulqor.toml"
            )))
        }
    }

    fn backend_available(cmds: &crate::commands::CommandRegistry, active: &str, action: &str) -> bool {
        cmds.list_commands()
            .iter()
            .any(|c| c == &Self::backend_key(active, action))
    }
}

impl Extension for ProviderRouter {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        register_info(self.active.clone(), &ctx.commands)?;
        register_set_active(self.active.clone(), &ctx.commands)?;
        for action in ACTIONS {
            register_proxy(
                self.active.clone(),
                action,
                ctx.commands.clone(),
            )?;
        }
        Ok(())
    }
}

fn register_info(
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "provider".into(), action: "info".into(), version: 1 },
            owner: "provider-router".into(),
            input_schema: "{}".into(),
            output_schema: r#"{ "active": "string", "providers": "array", "available": "array" }"#
                .into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |_| {
            let active_id = active.read().unwrap().clone();
            let available: Vec<String> = PROVIDERS
                .iter()
                .filter(|p| {
                    ProviderRouter::backend_available(&cmds_for_handler, p.id, "connect")
                })
                .map(|p| p.id.to_owned())
                .collect();
            let providers: Vec<serde_json::Value> = PROVIDERS
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "id": p.id,
                        "label": p.label,
                        "default_url": p.default_url,
                        "extension": p.extension_id,
                        "vram_hint": p.vram_hint,
                        "enabled": available.iter().any(|id| id == p.id),
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "active": active_id,
                "providers": providers,
                "available": available,
            }))
        }),
    )
}

fn register_set_active(
    active: Arc<RwLock<String>>,
    cmds: &Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId { namespace: "provider".into(), action: "set-active".into(), version: 1 },
            owner: "provider-router".into(),
            input_schema: r#"{ "provider": "string" }"#.into(),
            output_schema: r#"{ "active": "string", "default_url": "string" }"#.into(),
            callable_by: vec!["panel".into(), "agent".into()],
            permission: Permission::Read,
        },
        Arc::new(move |input| {
            let id = input["provider"]
                .as_str()
                .ok_or_else(|| CoreError::Io("set-active: 'provider' field required".into()))?
                .trim()
                .to_owned();
            let meta = PROVIDERS
                .iter()
                .find(|p| p.id == id)
                .ok_or_else(|| CoreError::Io(format!("unknown provider '{id}'")))?;
            if !ProviderRouter::backend_available(&cmds_for_handler, meta.id, "connect") {
                return Err(CoreError::UnknownCommand(format!(
                    "'{}' backend not loaded — enable {} in nulqor.toml",
                    meta.id, meta.extension_id
                )));
            }
            *active.write().unwrap() = meta.id.to_owned();
            Ok(serde_json::json!({
                "active": meta.id,
                "default_url": meta.default_url,
            }))
        }),
    )
}

fn register_proxy(
    active: Arc<RwLock<String>>,
    action: &str,
    cmds: Arc<crate::commands::CommandRegistry>,
) -> Result<(), CoreError> {
    let action_owned = action.to_owned();
    let permission = if action == "generate" {
        Permission::Write
    } else {
        Permission::Read
    };
    let callable_by = if action == "generate" {
        vec!["panel".into(), "agent".into(), "service".into()]
    } else {
        vec!["panel".into(), "agent".into()]
    };

    let cmds_for_handler = cmds.clone();
    cmds.register(
        CommandDecl {
            id: CommandId {
                namespace: "provider".into(),
                action: action_owned.clone(),
                version: 1,
            },
            owner: "provider-router".into(),
            input_schema: "{}".into(),
            output_schema: "{}".into(),
            callable_by,
            permission,
        },
        Arc::new(move |input| {
            let active_id = active.read().unwrap().clone();
            let backend = ProviderRouter::backend_key(&active_id, &action_owned);
            let backend_id = CommandId::parse(&backend).map_err(|_| {
                CoreError::UnknownCommand(format!("invalid backend key '{backend}'"))
            })?;
            if !ProviderRouter::backend_available(&cmds_for_handler, &active_id, &action_owned) {
                let meta = PROVIDERS
                    .iter()
                    .find(|p| p.id == active_id)
                    .map(|p| p.extension_id)
                    .unwrap_or("provider backend");
                return Err(CoreError::UnknownCommand(format!(
                    "'{backend}' not registered — enable {meta} in nulqor.toml enabled_extensions"
                )));
            }
            cmds_for_handler.invoke("provider-router", &backend_id, input)
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capability::{CapabilityRegistry, Capabilities},
        commands::CommandRegistry,
        context::InMemoryConfigStore,
        events::EventBus,
        permission::PermissionGate,
        types::ExtensionKind,
        version::VersionManager,
    };

    fn make_context() -> CoreContext {
        let perms = Arc::new(PermissionGate::new());
        CoreContext {
            bus: Arc::new(EventBus::new()),
            commands: Arc::new(CommandRegistry::new(perms.clone())),
            versions: Arc::new(VersionManager::new()),
            permissions: perms,
            caps: Arc::new(Capabilities::new()),
            capability_registry: Arc::new(CapabilityRegistry::new()),
            runtime: Arc::new(crate::runtime::Runtime::new()),
            config: Arc::new(InMemoryConfigStore::new()),
        }
    }

    fn router_manifest() -> ExtensionManifest {
        ExtensionManifest {
            id: "provider-router".into(),
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

    #[test]
    fn router_registers_public_provider_commands() {
        let ctx = make_context();
        let router = ProviderRouter::new(router_manifest(), "lmstudio".into());
        router.activate(&ctx).unwrap();
        let cmds = ctx.commands.list_commands();
        assert!(cmds.iter().any(|c| c == "provider:connect@1"));
        assert!(cmds.iter().any(|c| c == "provider:info@1"));
    }
}
