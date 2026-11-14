//! The engine's error type.

use serde::{Deserialize, Serialize};

pub type IdeResult<T> = Result<T, IdeError>;

/// An error with a kind a front end can switch on and a message to show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdeError {
    pub kind: ErrorKind,
    pub message: String,
}

impl std::fmt::Display for IdeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for IdeError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    NotFound,
    AlreadyExists,
    PermissionDenied,
    InvalidInput,
    /// Document version mismatch.
    OutOfSync,
    Cancelled,
    TimedOut,
    Unavailable,
    Io,
    Protocol,
    Internal,
}

impl IdeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self { kind, message: message.into() }
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, m)
    }
    pub fn invalid(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, m)
    }
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, m)
    }
    pub fn unavailable(m: impl Into<String>) -> Self {
        Self::new(ErrorKind::Unavailable, m)
    }
    pub fn cancelled() -> Self {
        Self::new(ErrorKind::Cancelled, "cancelled")
    }
}

impl From<std::io::Error> for IdeError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind as Io;
        let kind = match e.kind() {
            Io::NotFound => ErrorKind::NotFound,
            Io::AlreadyExists => ErrorKind::AlreadyExists,
            Io::PermissionDenied => ErrorKind::PermissionDenied,
            Io::InvalidInput | Io::InvalidData => ErrorKind::InvalidInput,
            _ => ErrorKind::Io,
        };
        Self::new(kind, e.to_string())
    }
}
