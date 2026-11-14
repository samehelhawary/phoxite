//! The engine's own vocabulary: text coordinates, edits, diagnostics, ids,
//! errors, events and settings.
//!
//! These types keep the engine independent of any one editor. A front end
//! (OpenIDEA's `ide-host`, the Phoxite language server `phoxite-lsp`) converts them at its
//! edge. Their shapes follow the LSP: 0-based lines, **UTF-16 columns**,
//! half-open ranges.

pub mod document;
pub mod error;
pub mod event;
pub mod inspection_settings;
pub mod settings;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

pub use document::Document;
pub use error::{ErrorKind, IdeError, IdeResult};
pub use event::{EventSink, IdeEvent, NullSink, ServerStatus, SharedSink};
/// Cooperative cancellation of long requests. The `tokio-util` token, so a
/// front end built on tokio passes its own tokens straight through.
pub use tokio_util::sync::CancellationToken;

/// A 0-based line / UTF-16 column position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

impl Pos {
    pub const fn new(line: u32, col: u32) -> Self {
        Self { line, col }
    }
}

/// Half-open range `[start, end)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Range {
    pub start: Pos,
    pub end: Pos,
}

impl Range {
    pub const fn new(start: Pos, end: Pos) -> Self {
        Self { start, end }
    }
    pub const fn point(p: Pos) -> Self {
        Self { start: p, end: p }
    }
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// A range inside a file.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Location {
    pub path: PathBuf,
    pub range: Range,
    /// Set for targets that are not files; the engine itself never sets it.
    pub uri: Option<String>,
}

impl Location {
    pub fn file(path: PathBuf, range: Range) -> Self {
        Self { path, range, uri: None }
    }
}

/// An edit in line/column coordinates (LSP `TextEdit`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// Edits to many files, applied together.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceEdit {
    pub files: Vec<FileEdit>,
}

/// One step of a [`WorkspaceEdit`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileEdit {
    Edit {
        path: PathBuf,
        edits: Vec<TextEdit>,
        /// The document version the edits apply to, when known.
        #[serde(default)]
        version: Option<i32>,
    },
    Create {
        path: PathBuf,
        overwrite: bool,
    },
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Delete {
        path: PathBuf,
        recursive: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A problem in a file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Severity,
    pub message: String,
    /// What produced it, e.g. `php`.
    pub source: Option<String>,
    /// The inspection id.
    pub code: Option<String>,
    #[serde(default)]
    pub unnecessary: bool,
    #[serde(default)]
    pub deprecated: bool,
    /// The service that reported it.
    #[serde(default)]
    pub server: Option<String>,
}

/// Symbol kinds, a stable subset of LSP `SymbolKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Interface,
    Struct,
    Enum,
    EnumMember,
    Trait,
    Function,
    Method,
    Constructor,
    Field,
    Property,
    Variable,
    Constant,
    TypeParameter,
    TypeAlias,
    Macro,
    Event,
    Operator,
    Key,
    Unknown,
}

impl SymbolKind {
    /// Class-likes: what *Go to Class* searches.
    pub fn is_type_like(self) -> bool {
        matches!(self, Self::Class | Self::Interface | Self::Struct | Self::Enum | Self::Trait | Self::TypeAlias)
    }
}

/// Declared visibility of a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    Public,
    Protected,
    Package,
    Private,
}

/// A node of a document outline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    /// The whole extent of the symbol.
    pub range: Range,
    /// The identifier itself.
    pub selection_range: Range,
    pub children: Vec<OutlineItem>,
    #[serde(default)]
    pub visibility: Option<Visibility>,
}

macro_rules! id_type {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            /// Allocates a process-unique id.
            pub fn next() -> Self {
                static NEXT: AtomicU32 = AtomicU32::new(1);
                Self(NEXT.fetch_add(1, Ordering::Relaxed))
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}#{}", stringify!($name), self.0)
            }
        }
    )*};
}

id_type!(
    /// An open document. The front end chooses the numbers.
    DocId,
    /// The id the engine reports itself under in [`IdeEvent::LanguageServer`].
    ServerId,
);
