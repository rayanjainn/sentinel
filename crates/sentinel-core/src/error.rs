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
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("this action preview has expired or was already used; review it again")]
    ActionTokenInvalid,
    #[error("operation cancelled")]
    Cancelled,
    #[error("network error: {message}")]
    Network { message: String },
    #[error("{provider} API error: {message}")]
    Provider {
        provider: String,
        status: Option<u16>,
        message: String,
    },
    #[error("i/o error: {message}")]
    Io {
        message: String,
        path: Option<String>,
    },
    #[error("internal error: {message}")]
    Internal { message: String },
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
                message: err.to_string(),
                path,
            },
        }
    }

    pub fn internal(message: impl Display) -> Self {
        Self::Internal {
            message: message.to_string(),
        }
    }

    pub fn invalid(message: impl Display) -> Self {
        Self::InvalidInput {
            message: message.to_string(),
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
