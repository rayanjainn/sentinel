use std::fmt::Display;
use std::io::ErrorKind;
use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::Pid;

pub type CoreResult<T> = Result<T, SentinelError>;

/// Every fallible backend call returns this. Variants carry enough context for the UI to render a
/// specific message ("Permission denied reading /Library/Caches — grant Full Disk Access") instead
/// of a generic failure. Serialized with a `code` discriminator.
///
/// No variant may have a field named `message`: [`ErrorPayload`] flattens this enum next to its own
/// `message`.
#[derive(Debug, Clone, PartialEq, thiserror::Error, Serialize, Deserialize, TS)]
#[serde(
    tag = "code",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SentinelError {
    #[error("permission denied: {operation}")]
    PermissionDenied {
        operation: String,
        target: Option<String>,
        hint: Option<String>,
    },
    #[error("process {pid} no longer exists")]
    ProcessNotFound { pid: Pid },
    /// PID was reused by a different process between preview and execution.
    #[error("process {pid} has been replaced by a different process")]
    ProcessChanged { pid: Pid },
    #[error("path not found: {path}")]
    PathNotFound { path: String },
    #[error("{feature} is not available on this system: {reason}")]
    Unavailable { feature: String, reason: String },
    /// User dismissed the OS administrator/UAC/polkit prompt.
    #[error("administrator authorization was declined for: {operation}")]
    ElevationDeclined { operation: String },
    #[error("invalid input: {detail}")]
    InvalidInput { detail: String },
    #[error("this action preview has expired or was already used; review it again")]
    ActionTokenInvalid,
    #[error("operation cancelled")]
    Cancelled,
    #[error("network error: {detail}")]
    Network { detail: String },
    #[error("{provider} API error: {detail}")]
    Provider {
        provider: String,
        status: Option<u16>,
        detail: String,
    },
    #[error("i/o error: {detail}")]
    Io {
        detail: String,
        path: Option<String>,
    },
    #[error("internal error: {detail}")]
    Internal { detail: String },
}

impl SentinelError {
    pub fn io(err: &std::io::Error, path: Option<&Path>) -> Self {
        let path = path.map(|p| p.to_string_lossy().into_owned());
        match (err.kind(), path) {
            (ErrorKind::PermissionDenied, target) => Self::PermissionDenied {
                operation: "access".into(),
                target,
                hint: None,
            },
            (ErrorKind::NotFound, Some(path)) => Self::PathNotFound { path },
            (_, path) => Self::Io {
                detail: err.to_string(),
                path,
            },
        }
    }

    pub fn internal(detail: impl Display) -> Self {
        Self::Internal {
            detail: detail.to_string(),
        }
    }

    pub fn invalid(detail: impl Display) -> Self {
        Self::InvalidInput {
            detail: detail.to_string(),
        }
    }
}

/// Wire shape sent to the frontend: the tagged error plus its rendered message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ErrorPayload {
    #[serde(flatten)]
    pub error: SentinelError,
    pub message: String,
}

impl From<SentinelError> for ErrorPayload {
    fn from(error: SentinelError) -> Self {
        let message = error.to_string();
        Self { error, message }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_json_has_single_message_key() {
        let payload = ErrorPayload::from(SentinelError::Network {
            detail: "offline".into(),
        });
        let json = serde_json::to_string(&payload).unwrap();
        assert_eq!(json.matches("\"message\"").count(), 1, "{json}");
        let back: ErrorPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(back, payload);
    }

    #[test]
    fn io_error_maps_permission_and_not_found() {
        let denied = std::io::Error::from(ErrorKind::PermissionDenied);
        assert!(matches!(
            SentinelError::io(&denied, Some(Path::new("/x"))),
            SentinelError::PermissionDenied { .. }
        ));
        let missing = std::io::Error::from(ErrorKind::NotFound);
        assert_eq!(
            SentinelError::io(&missing, Some(Path::new("/x"))),
            SentinelError::PathNotFound { path: "/x".into() }
        );
    }
}
