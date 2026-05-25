//! Core error type. All failures surface here — never swallowed.

use crate::types::Permission;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("version mismatch: wanted '{wanted}', available: {available:?}")]
    VersionMismatch { wanted: String, available: Vec<String> },

    #[error("unknown command: '{0}'")]
    UnknownCommand(String),

    #[error("unknown capability: kind='{kind}' instance='{instance}'")]
    UnknownCapability { kind: String, instance: String },

    #[error("permission denied: caller='{caller}' needs {needed:?} for '{what}'")]
    PermissionDenied { caller: String, needed: Permission, what: String },

    #[error("boundary violation: {0}")]
    BoundaryViolation(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("linter error:\n{0}")]
    Linter(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("extension not found in static registry: '{0}'")]
    ExtensionNotFound(String),

    #[error("dependency cycle detected involving: {0}")]
    DependencyCycle(String),

    #[error("duplicate capability: kind='{capability}' instance='{instance}' already registered")]
    DuplicateCapability { capability: String, instance: String },

    #[error("extension activation error in '{ext_id}': {source}")]
    Activation { ext_id: String, source: Box<CoreError> },
}
