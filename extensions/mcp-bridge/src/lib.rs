//! MCP bridge extension — Phase 2.6 (BUILD_PLAN §2.6, decisions/006 §9).
//!
//! Thin stdio MCP proxy to the HTTP API. The Tauri app MUST be running.
//! Exposes exactly the five tools from decisions/006 §9:
//!   register_observer, catch_up, ack_observer, send_message, list_observers
//!
//! MCP transport: stdio (Cursor/Windsurf IDE config).
//! The bridge reads NULQOR_API_URL from the environment (default: http://localhost:8787).
//!
//! This Rust extension registers the bridge activation command. The actual
//! stdio MCP server binary is spawned as a sidecar (Phase 4+). For Phase 2,
//! the bridge exposes the five tools as core commands callable by external agents.

use std::sync::{Arc, OnceLock};

use crate::capability::Capabilities;
use crate::context::{CoreContext, Extension};
use crate::error::CoreError;
use crate::runtime::Runtime;
use crate::types::{CommandDecl, CommandId, ExtensionManifest, Permission};

pub struct McpBridgeExtension {
    manifest: ExtensionManifest,
}

impl McpBridgeExtension {
    pub fn new(manifest: ExtensionManifest) -> Self {
        Self { manifest }
    }

    fn api_url(caps: &Capabilities, ext_id: &str) -> Result<String, CoreError> {
        let url = std::env::var("NULQOR_API_URL")
            .unwrap_or_else(|_| "http://localhost:8787".into());
        caps.check_http_allowed(ext_id, &url)?;
        Ok(url)
    }
}

impl Extension for McpBridgeExtension {
    fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        let caps = ctx.caps.clone();
        let runtime = ctx.runtime.clone();

        // mcp-bridge:register-observer@1
        {
            let caps = caps.clone();
            let runtime = runtime.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "mcp-bridge".into(),
                        action: "register-observer".into(),
                        version: 1,
                    },
                    owner: "mcp-bridge".into(),
                    input_schema: r#"{ "name": "string?" }"#.into(),
                    output_schema: r#"{ "name": "string", "last_ack_seq": "number" }"#.into(),
                    callable_by: vec!["agent".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let base = Self::api_url(&caps, "mcp-bridge")?;
                    let name = input["name"].as_str().unwrap_or("").to_owned();
                    http_post_sync(&runtime, &format!("{base}/observers/register"), serde_json::json!({ "name": name }))
                }),
            )?;
        }

        // mcp-bridge:catch-up@1
        {
            let caps = caps.clone();
            let runtime = runtime.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "mcp-bridge".into(),
                        action: "catch-up".into(),
                        version: 1,
                    },
                    owner: "mcp-bridge".into(),
                    input_schema: r#"{ "observer_name": "string", "auto_ack": "boolean?" }"#.into(),
                    output_schema: r#"{ "events": "array" }"#.into(),
                    callable_by: vec!["agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |input| {
                    let base = Self::api_url(&caps, "mcp-bridge")?;
                    let name = input["observer_name"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("catch-up: observer_name required".into()))?;
                    let auto_ack = input["auto_ack"].as_bool().unwrap_or(false);
                    let encoded_name = name.replace('%', "%25").replace(' ', "%20").replace('&', "%26").replace('=', "%3D").replace('+', "%2B").replace('#', "%23");
                    http_get_sync(&runtime, &format!(
                        "{base}/observers/catch-up?observer={encoded_name}&auto_ack={auto_ack}"
                    ))
                }),
            )?;
        }

        // mcp-bridge:ack-observer@1
        {
            let caps = caps.clone();
            let runtime = runtime.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "mcp-bridge".into(),
                        action: "ack-observer".into(),
                        version: 1,
                    },
                    owner: "mcp-bridge".into(),
                    input_schema: r#"{ "observer_name": "string" }"#.into(),
                    output_schema: r#"{ "ok": "boolean" }"#.into(),
                    callable_by: vec!["agent".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let base = Self::api_url(&caps, "mcp-bridge")?;
                    let name = input["observer_name"]
                        .as_str()
                        .ok_or_else(|| CoreError::Io("ack-observer: observer_name required".into()))?
                        .to_owned();
                    http_post_sync(&runtime, &format!("{base}/observers/ack"), serde_json::json!({ "name": name }))
                }),
            )?;
        }

        // mcp-bridge:send-message@1
        {
            let caps = caps.clone();
            let runtime = runtime.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "mcp-bridge".into(),
                        action: "send-message".into(),
                        version: 1,
                    },
                    owner: "mcp-bridge".into(),
                    input_schema: r#"{ "message": "string", "observer_name": "string", "model": "string?", "agent": "string?" }"#.into(),
                    output_schema: r#"{ "stream_id": "string" }"#.into(),
                    callable_by: vec!["agent".into()],
                    permission: Permission::Write,
                },
                Arc::new(move |input| {
                    let base = Self::api_url(&caps, "mcp-bridge")?;
                    http_post_sync(&runtime, &format!("{base}/message"), input)
                }),
            )?;
        }

        // mcp-bridge:list-observers@1
        {
            let caps = caps.clone();
            let runtime = runtime.clone();
            ctx.commands.register(
                CommandDecl {
                    id: CommandId {
                        namespace: "mcp-bridge".into(),
                        action: "list-observers".into(),
                        version: 1,
                    },
                    owner: "mcp-bridge".into(),
                    input_schema: "{}".into(),
                    output_schema: r#"{ "observers": "array" }"#.into(),
                    callable_by: vec!["agent".into()],
                    permission: Permission::Read,
                },
                Arc::new(move |_| {
                    let base = Self::api_url(&caps, "mcp-bridge")?;
                    http_get_sync(&runtime, &format!("{base}/observers"))
                }),
            )?;
        }

        eprintln!("[mcp-bridge] activated — API URL: {}",
            std::env::var("NULQOR_API_URL").unwrap_or_else(|_| "http://localhost:8787".into())
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Sync HTTP helpers (block_in_place over reqwest)
// ---------------------------------------------------------------------------

/// Shared HTTP client with a 5 s connect timeout and 30 s read/total timeout.
/// Built once and reused across all calls; avoids per-request client construction
/// and ensures requests do not block indefinitely when the API is unreachable.
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build MCP-bridge HTTP client")
    })
}

fn http_get_sync(runtime: &Runtime, url: &str) -> Result<serde_json::Value, CoreError> {
    let url = url.to_owned();
    runtime.block_on_compat(async move {
        let client = shared_client();
        let resp = client
            .get(&url)
            .send()
            .await
            .map_err(|e| CoreError::Io(format!("GET {url}: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Io(format!("HTTP {status}: {body}").into()));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| CoreError::Io(format!("GET {url} parse: {e}")))
    })
}

fn http_post_sync(runtime: &Runtime, url: &str, body: serde_json::Value) -> Result<serde_json::Value, CoreError> {
    let url = url.to_owned();
    runtime.block_on_compat(async move {
        let client = shared_client();
        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| CoreError::Io(format!("POST {url}: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(CoreError::Io(format!("HTTP {status}: {body}").into()));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| CoreError::Io(format!("POST {url} parse: {e}")))
    })
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
            id: "mcp-bridge".into(),
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
            http_hosts: vec!["localhost".into()],
        }
    }

    #[test]
    fn mcp_bridge_registers_five_commands() {
        let ctx = make_context();
        ctx.caps.register_scopes("mcp-bridge", vec![], vec!["localhost".into()]);
        let ext = McpBridgeExtension::new(make_manifest());
        ext.activate(&ctx).expect("activate");

        let cmds = ctx.commands.list_commands();
        assert!(cmds.iter().any(|c| c == "mcp-bridge:register-observer@1"));
        assert!(cmds.iter().any(|c| c == "mcp-bridge:catch-up@1"));
        assert!(cmds.iter().any(|c| c == "mcp-bridge:ack-observer@1"));
        assert!(cmds.iter().any(|c| c == "mcp-bridge:send-message@1"));
        assert!(cmds.iter().any(|c| c == "mcp-bridge:list-observers@1"));
    }

    #[test]
    fn mcp_bridge_rejects_non_localhost_url() {
        let ctx = make_context();
        ctx.caps.register_scopes("mcp-bridge", vec![], vec!["localhost".into()]);
        let ext = McpBridgeExtension::new(make_manifest());
        ext.activate(&ctx).unwrap();

        // Override API URL to something not in declared hosts
        std::env::set_var("NULQOR_API_URL", "http://evil.example.com:8080");
        let result = ctx.commands.invoke(
            "test",
            &CommandId::parse("mcp-bridge:list-observers@1").unwrap(),
            serde_json::json!({}),
        );
        std::env::remove_var("NULQOR_API_URL");
        assert!(
            matches!(result, Err(CoreError::BoundaryViolation(_))),
            "expected BoundaryViolation for external host: {result:?}"
        );
    }
}
