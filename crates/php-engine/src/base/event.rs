//! What the engine reports while it runs: problems, indexing, its own status.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::{Diagnostic, ServerId};

/// An event the engine publishes through its [`EventSink`].
#[derive(Clone, Debug, PartialEq)]
pub enum IdeEvent {
    /// The problems of one file from one source, replacing that source's
    /// earlier ones for the file.
    Diagnostics { path: PathBuf, source: String, items: Vec<Diagnostic> },
    /// Indexing started (`indexing: true`) or finished.
    IndexState { indexing: bool, files: u32, symbols: u32 },
    /// The engine's own status, as a language server's would be.
    LanguageServer { server: ServerId, name: String, status: ServerStatus, message: Option<String> },
    /// The engine shut down.
    LanguageServerRemoved { server: ServerId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerStatus {
    Starting,
    Ready,
    Busy,
    Crashed,
    Stopped,
    Missing,
    Misconfigured,
}

/// Where the engine publishes its events.
pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: IdeEvent);
}

impl<F: Fn(IdeEvent) + Send + Sync + 'static> EventSink for F {
    fn emit(&self, event: IdeEvent) {
        self(event)
    }
}

pub type SharedSink = Arc<dyn EventSink>;

/// Discards events; for tests and tools.
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _: IdeEvent) {}
}
