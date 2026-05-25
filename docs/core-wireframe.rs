// ============================================================================
// Nulqor core — WIREFRAME / SKELETON (reference for building agents)
// ============================================================================
// This is NOT the final implementation. It is the SHAPE the core must take, so
// building agents fill in bodies instead of inventing structure. Types, traits,
// and signatures here are authoritative; bodies marked `todo!()` are yours.
//
// Maps 1:1 to DESIGN.md §2 (the eight responsibilities) and the four ADRs.
// Keep this file as docs; split into src-tauri/src/*.rs per DESIGN.md §14 when
// implementing. Do NOT add product behavior here — see decisions/001.
// ============================================================================

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Shared identifiers — versioned by construction (ADR 002, 003)
// ---------------------------------------------------------------------------

/// A command id: "namespace:action@version", e.g. "wiki:get-page@1".
/// Parsing MUST reject anything without all three parts. Fail loud.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CommandId {
    pub namespace: String,
    pub action: String,
    pub version: u32,
}

/// An event id: "namespace:name@version", e.g. "canvas:layout-saved@1".
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EventId {
    pub namespace: String,
    pub name: String,
    pub version: u32,
}

/// The four permission classes (DESIGN.md §5). Enforced on every invocation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Permission { Read, Write, Destructive, System }

/// All core errors are typed and surfaced — never swallowed (quality gates §13).
#[derive(Debug)]
pub enum CoreError {
    VersionMismatch { wanted: String, available: Vec<String> },
    UnknownCommand(CommandId),
    UnknownCapability { kind: String, instance: String },
    PermissionDenied { id: CommandId, needed: Permission },
    BoundaryViolation(String),
    Timeout(String),
    Linter(String),
    Io(String),
}

// ---------------------------------------------------------------------------
// 1. Extension contract + loader (DESIGN.md §2.1)
// ---------------------------------------------------------------------------

/// Parsed from extension.toml. The loader builds this; the linter validates it
/// BEFORE the extension is loaded. Static declaration only (ADR/ §12): every
/// command/event the extension references is listed here, never built at runtime.
pub struct ExtensionManifest {
    pub id: String,
    pub version: semver::Version,
    pub kind: ExtensionKind,
    pub api_version: String,        // "v1"
    pub schema_version: semver::Version,
    pub min_core_version: semver::Version,
    pub requires: Vec<String>,      // hard deps -> load order
    pub optional: Vec<String>,
    pub provides: Vec<CapabilityDecl>,   // slotted/additive capabilities (§5)
    pub commands: Vec<CommandDecl>,
    pub publishes: Vec<EventId>,         // for bus filtering + bake graph
    pub subscribes: Vec<EventPattern>,
    pub fs_scopes: Vec<String>,          // capability layer scope (§7)
    pub http_hosts: Vec<String>,
}

pub enum ExtensionKind { Panel, Host, Service, Provider, Tool, Theme, Bake }

/// Every extension implements this. The core calls these — nothing else.
/// Keep it tiny: the core hosts; it does not understand what the extension does.
pub trait Extension: Send + Sync {
    fn manifest(&self) -> &ExtensionManifest;
    /// Register commands/panels/capabilities/subscriptions via `ctx`. Called once.
    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError>;
    /// Optional clean shutdown.
    fn deactivate(&self, _ctx: &CoreContext) -> Result<(), CoreError> { Ok(()) }
}

/// The loader: discover -> lint -> dependency-order -> load -> activate.
/// Lazy activation: an extension may be loaded but activated on first need.
pub struct Loader { /* registry handle, linter handle */ }
impl Loader {
    pub fn scan_and_load(&self, _ctx: &CoreContext) -> Result<(), CoreError> {
        // 1. scan extensions/ for extension.toml
        // 2. run linter on each (reject on any error, BEFORE load)
        // 3. topologically sort by `requires` (cycle = loud error)
        // 4. load + call activate(ctx) in order
        todo!()
    }
}

// ---------------------------------------------------------------------------
// 2. Event bus — namespace-scoped delivery (ADR 003, DESIGN.md §6)
// ---------------------------------------------------------------------------

/// A subscription pattern. Delivery matches on namespace (+ optional name/version
/// range). Non-matching subscribers are NEVER woken — no broadcast-then-discard.
pub struct EventPattern {
    pub namespace: String,
    pub name: Option<String>,
    pub version_range: Option<(u32, u32)>,
}

pub struct NamespacedEvent { pub id: EventId, pub payload: serde_json::Value }
pub struct Subscription { /* opaque handle; drop to unsubscribe */ }

pub trait EventBus: Send + Sync {
    fn publish(&self, ev: NamespacedEvent) -> Result<(), CoreError>;
    fn subscribe(
        &self,
        pattern: EventPattern,
        handler: Arc<dyn Fn(&NamespacedEvent) + Send + Sync>,
    ) -> Subscription;
    // Implementation note: index subscribers by namespace so publish() only
    // touches matching handlers. This is the line that keeps 100 exts quiet.
}

// ---------------------------------------------------------------------------
// 3. Command registry — versioned, owned, permissioned (DESIGN.md §5)
// ---------------------------------------------------------------------------

pub struct CommandDecl {
    pub id: CommandId,
    pub owner: String,
    pub input_schema: String,
    pub output_schema: String,
    pub callable_by: Vec<String>,   // ["agent","panel","service",...]
    pub permission: Permission,
}

pub trait CommandRegistry: Send + Sync {
    fn register(
        &self,
        decl: CommandDecl,
        handler: Arc<dyn Fn(serde_json::Value) -> Result<serde_json::Value, CoreError> + Send + Sync>,
    ) -> Result<(), CoreError>;
    /// Invoke a SPECIFIC version. Missing version -> VersionMismatch (no fallback).
    /// Permission checked here via the gate before the handler runs.
    fn invoke(&self, caller: &str, id: &CommandId, input: serde_json::Value)
        -> Result<serde_json::Value, CoreError>;
}

// ---------------------------------------------------------------------------
// 4. Version manager — three axes + per-contract coexistence (ADR 002)
// ---------------------------------------------------------------------------

pub struct VersionManifest {
    pub core_version: semver::Version,
    pub api_versions_supported: Vec<String>,         // ["v1"], grows to ["v1","v2"]
    pub schema_version: semver::Version,
    pub loaded_extensions: HashMap<String, semver::Version>,
    pub live_command_versions: HashMap<String, Vec<u32>>, // "wiki:get-page" -> [1,2]
}

pub trait VersionManager: Send + Sync {
    /// Called at load. Rejects an extension whose api/schema/min-core needs are
    /// unmet — loud, with a compatibility report, BEFORE anything renders.
    fn check_extension(&self, m: &ExtensionManifest) -> Result<(), CoreError>;
    /// Who still depends on a given command version? (drives safe @1 retirement)
    fn dependents_of(&self, id: &CommandId) -> Vec<String>;
    fn manifest(&self) -> VersionManifest;
}

// ---------------------------------------------------------------------------
// 5. Permission gate (DESIGN.md §5)
// ---------------------------------------------------------------------------

pub trait PermissionGate: Send + Sync {
    /// Read: allow. Write: allow + log. Destructive: require confirmation hook.
    /// System: restricted to core-trusted callers.
    fn check(&self, caller: &str, needed: Permission, what: &str) -> Result<(), CoreError>;
    fn set_confirm_hook(&self, hook: Arc<dyn Fn(&str) -> bool + Send + Sync>);
}

// ---------------------------------------------------------------------------
// 6. Capability layer — the ONLY door to fs/net/process (DESIGN.md §7, ADR 004)
// ---------------------------------------------------------------------------

pub struct SidecarSpec { pub program: String, pub args: Vec<String>, pub timeout: Duration }
pub struct SidecarHandle { /* pid, kill switch, stdout/stderr streams */ }

pub trait Capabilities: Send + Sync {
    fn fs_read(&self, ext_id: &str, scoped_path: &str) -> Result<Vec<u8>, CoreError>;
    fn fs_write(&self, ext_id: &str, scoped_path: &str, bytes: &[u8]) -> Result<(), CoreError>;
    fn http_request(&self, ext_id: &str, req: HttpRequest) -> Result<HttpResponse, CoreError>;
    /// system-permission only. Core owns lifecycle: timeout, kill, loud-fail on hang.
    fn spawn_sidecar(&self, ext_id: &str, spec: SidecarSpec) -> Result<SidecarHandle, CoreError>;
    // fs_* must reject any path outside the extension's declared scope -> BoundaryViolation.
}
pub struct HttpRequest { /* method, url, headers, body */ }
pub struct HttpResponse { /* status, headers, body */ }

// ---------------------------------------------------------------------------
// 7. Async runtime owner (ADR 004, DESIGN.md §8)
// ---------------------------------------------------------------------------

pub trait Runtime: Send + Sync {
    /// Async I/O work on the shared runtime. Cancellable; carries a timeout budget.
    fn spawn_task(&self, budget: Duration, fut: futures::future::BoxFuture<'static, ()>);
    /// CPU-bound work OFF the async runtime, on a separate pool. Hook exists now;
    /// heavy consumers (eval scoring, embeddings) arrive in a later phase.
    fn spawn_compute<T: Send + 'static>(&self, job: Box<dyn FnOnce() -> T + Send>)
        -> std::thread::JoinHandle<T>;
}

// ---------------------------------------------------------------------------
// 8. Capabilities resolution — named instances (DESIGN.md §5, the seam-2 fix)
// ---------------------------------------------------------------------------

pub struct CapabilityDecl {
    pub capability: String,   // slot kind: "storage" | "provider" | "memory" | additive kinds
    pub instance: String,     // named instance on the shelf: "main" | "analytics" | "lmstudio"
    pub contract: String,     // "storage@1", "provider@1"
}

pub trait CapabilityRegistry: Send + Sync {
    fn provide(&self, ext_id: &str, decl: CapabilityDecl, handle: Arc<dyn std::any::Any + Send + Sync>)
        -> Result<(), CoreError>;   // duplicate (capability,instance) -> loud conflict
    /// Ask by (kind, instance, contract-version). Missing -> UnknownCapability. No guessing.
    fn resolve(&self, capability: &str, instance: &str, contract: &str)
        -> Result<Arc<dyn std::any::Any + Send + Sync>, CoreError>;
}

// ---------------------------------------------------------------------------
// CoreContext — the ONE handle every extension receives in activate().
// This is the entire surface an extension may touch. Nothing else is reachable.
// ---------------------------------------------------------------------------

pub struct CoreContext {
    pub bus: Arc<dyn EventBus>,
    pub commands: Arc<dyn CommandRegistry>,
    pub versions: Arc<dyn VersionManager>,
    pub permissions: Arc<dyn PermissionGate>,
    pub caps: Arc<dyn Capabilities>,
    pub capability_registry: Arc<dyn CapabilityRegistry>,
    pub runtime: Arc<dyn Runtime>,
    pub config: Arc<dyn ConfigStore>,
}

pub trait ConfigStore: Send + Sync {
    fn get(&self, ext_id: &str, key: &str) -> Result<serde_json::Value, CoreError>;
    fn set(&self, ext_id: &str, key: &str, value: serde_json::Value) -> Result<(), CoreError>;
}

// ============================================================================
// EXAMPLE: what a real extension looks like against this core (the sample panel,
// BUILD_PLAN 1.10). Shows the intended shape — provider/transcript/etc. follow it.
// ============================================================================

pub struct HelloPanel { manifest: ExtensionManifest }

impl Extension for HelloPanel {
    fn manifest(&self) -> &ExtensionManifest { &self.manifest }
    fn activate(&self, ctx: &CoreContext) -> Result<(), CoreError> {
        // register one read command
        ctx.commands.register(
            CommandDecl {
                id: CommandId { namespace: "hello".into(), action: "ping".into(), version: 1 },
                owner: "hello-panel".into(),
                input_schema: "{}".into(),
                output_schema: "{ pong: bool }".into(),
                callable_by: vec!["panel".into(), "agent".into()],
                permission: Permission::Read,
            },
            Arc::new(|_input| Ok(serde_json::json!({ "pong": true }))),
        )?;
        // subscribe to one event, scoped to the canvas namespace
        let _sub = ctx.bus.subscribe(
            EventPattern { namespace: "canvas".into(), name: Some("ready".into()), version_range: None },
            Arc::new(|ev| { let _ = ev; /* mount the panel UI via IPC */ }),
        );
        Ok(())
    }
}
