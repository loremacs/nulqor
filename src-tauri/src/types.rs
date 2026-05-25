//! Shared primitive types used across all core modules.
//! Kept free of cross-module dependencies so every module can import this safely.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Command and event identifiers — versioned by construction (DESIGN.md §4)
// ---------------------------------------------------------------------------

/// A command identifier: `namespace:action@version` (e.g. `hello:ping@1`).
/// Parsing enforces that all three parts are present. Fail loud if not.
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct CommandId {
    pub namespace: String,
    pub action: String,
    pub version: u32,
}

impl CommandId {
    /// Parse from `"namespace:action@version"`. Returns `Err` with a human message on failure.
    pub fn parse(s: &str) -> Result<Self, String> {
        let at = s
            .rfind('@')
            .ok_or_else(|| format!("missing @version in command id '{s}'"))?;
        let prefix = &s[..at];
        let version: u32 = s[at + 1..]
            .parse()
            .map_err(|_| format!("version after '@' must be u32 in '{s}'"))?;
        if version == 0 {
            return Err(format!("command id version must be >= 1 in '{s}'"));
        }
        let colon = prefix
            .find(':')
            .ok_or_else(|| format!("missing ':' in command id '{s}'"))?;
        Ok(Self {
            namespace: prefix[..colon].to_owned(),
            action: prefix[colon + 1..].to_owned(),
            version,
        })
    }

    /// Canonical string key for registry lookups.
    pub fn key(&self) -> String {
        format!("{}:{}@{}", self.namespace, self.action, self.version)
    }

    /// Base key without version (used for listing all versions of a command).
    pub fn base_key(&self) -> String {
        format!("{}:{}", self.namespace, self.action)
    }
}

impl std::fmt::Display for CommandId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

/// An event identifier: `namespace:name@version` (e.g. `canvas:ready@1`).
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct EventId {
    pub namespace: String,
    pub name: String,
    pub version: u32,
}

impl EventId {
    pub fn parse(s: &str) -> Result<Self, String> {
        let at = s
            .rfind('@')
            .ok_or_else(|| format!("missing @version in event id '{s}'"))?;
        let prefix = &s[..at];
        let version: u32 = s[at + 1..]
            .parse()
            .map_err(|_| format!("version after '@' must be u32 in '{s}'"))?;
        let colon = prefix
            .find(':')
            .ok_or_else(|| format!("missing ':' in event id '{s}'"))?;
        Ok(Self {
            namespace: prefix[..colon].to_owned(),
            name: prefix[colon + 1..].to_owned(),
            version,
        })
    }

    pub fn key(&self) -> String {
        format!("{}:{}@{}", self.namespace, self.name, self.version)
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.key())
    }
}

// ---------------------------------------------------------------------------
// Permission classes (DESIGN.md §5)
// ---------------------------------------------------------------------------

/// The four permission classes enforced on every command invocation and capability request.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    /// Safe read-only access. No confirmation needed.
    Read,
    /// Modifies state; logged.
    Write,
    /// Requires explicit human confirmation before proceeding.
    Destructive,
    /// Core-level / restricted. Only trusted system callers.
    System,
}

// ---------------------------------------------------------------------------
// Event subscription pattern
// ---------------------------------------------------------------------------

/// Matches events by namespace (required) and optionally by name and version range.
/// Only matching subscribers are woken — non-matching ones are never called.
#[derive(Clone, Debug)]
pub struct EventPattern {
    pub namespace: String,
    pub name: Option<String>,
    /// Inclusive [min, max] version range. `None` matches all versions.
    pub version_range: Option<(u32, u32)>,
}

impl EventPattern {
    pub fn namespace(ns: &str) -> Self {
        Self { namespace: ns.to_owned(), name: None, version_range: None }
    }

    pub fn exact(ns: &str, name: &str, version: u32) -> Self {
        Self {
            namespace: ns.to_owned(),
            name: Some(name.to_owned()),
            version_range: Some((version, version)),
        }
    }

    /// Returns `true` if this pattern matches the given event id.
    pub fn matches(&self, id: &EventId) -> bool {
        if id.namespace != self.namespace {
            return false;
        }
        if let Some(n) = &self.name {
            if &id.name != n {
                return false;
            }
        }
        if let Some((lo, hi)) = self.version_range {
            if id.version < lo || id.version > hi {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Event payload
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamespacedEvent {
    pub id: EventId,
    pub payload: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Command declaration (registered by extensions)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CommandDecl {
    pub id: CommandId,
    pub owner: String,
    pub input_schema: String,
    pub output_schema: String,
    pub callable_by: Vec<String>,
    pub permission: Permission,
}

// ---------------------------------------------------------------------------
// Capability declaration (provided by extensions — DESIGN.md §5)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct CapabilityDecl {
    /// Slot kind: `"storage"`, `"provider"`, `"memory"`, etc.
    pub capability: String,
    /// Named instance on the shelf: `"main"`, `"lmstudio"`, etc.
    pub instance: String,
    /// Contract version: `"storage@1"`, `"provider@1"`.
    pub contract: String,
}

// ---------------------------------------------------------------------------
// Extension manifest (fully typed — constructed from the parsed TOML)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum ExtensionKind {
    Panel,
    Host,
    Service,
    Provider,
    Tool,
    Theme,
    Bake,
}

impl ExtensionKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "Panel" => Some(Self::Panel),
            "Host" => Some(Self::Host),
            "Service" => Some(Self::Service),
            "Provider" => Some(Self::Provider),
            "Tool" => Some(Self::Tool),
            "Theme" => Some(Self::Theme),
            "Bake" => Some(Self::Bake),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExtensionManifest {
    pub id: String,
    pub version: semver::Version,
    pub kind: ExtensionKind,
    pub api_version: String,
    pub schema_version: semver::Version,
    pub min_core: semver::Version,
    pub requires: Vec<String>,
    pub optional: Vec<String>,
    pub provides: Vec<CapabilityDecl>,
    pub commands: Vec<CommandDecl>,
    pub publishes: Vec<EventId>,
    pub subscribes: Vec<EventPattern>,
    pub fs_scopes: Vec<String>,
    pub http_hosts: Vec<String>,
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// HTTP/Sidecar stubs (capability layer placeholders for Phase 1+)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

#[allow(dead_code)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: std::collections::HashMap<String, String>,
    pub body: Vec<u8>,
}

#[allow(dead_code)]
pub struct SidecarSpec {
    pub program: String,
    pub args: Vec<String>,
    pub timeout: std::time::Duration,
}

#[allow(dead_code)]
pub struct SidecarHandle {
    pub pid: u32,
    // Kill switch, stdout/stderr streams added in Phase 4+
}
