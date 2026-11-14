//! Conversions between the engine's types (`php_engine::base`) and the LSP's.
//!
//! Both count lines from 0 and columns in UTF-16 code units, so positions
//! map one to one.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lsp_types as lsp;
use lsp_types::Url;
use php_engine::base as e;

pub fn pos(p: lsp::Position) -> e::Pos {
    e::Pos::new(p.line, p.character)
}

pub fn lsp_pos(p: e::Pos) -> lsp::Position {
    lsp::Position::new(p.line, p.col)
}

pub fn range(r: lsp::Range) -> e::Range {
    e::Range::new(pos(r.start), pos(r.end))
}

pub fn lsp_range(r: e::Range) -> lsp::Range {
    lsp::Range::new(lsp_pos(r.start), lsp_pos(r.end))
}

/// The file a `file:` URI names, without Windows' `\\?\` prefix.
pub fn path_of(uri: &Url) -> Option<PathBuf> {
    let path = uri.to_file_path().ok()?;
    Some(dunce::simplified(&path).to_path_buf())
}

pub fn uri_of(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}

pub fn location(path: &Path, r: e::Range) -> Option<lsp::Location> {
    Some(lsp::Location::new(uri_of(path)?, lsp_range(r)))
}

pub fn text_edit(edit: e::TextEdit) -> lsp::TextEdit {
    lsp::TextEdit::new(lsp_range(edit.range), edit.new_text)
}

pub fn severity(s: e::Severity) -> lsp::DiagnosticSeverity {
    match s {
        e::Severity::Error => lsp::DiagnosticSeverity::ERROR,
        e::Severity::Warning => lsp::DiagnosticSeverity::WARNING,
        e::Severity::Info => lsp::DiagnosticSeverity::INFORMATION,
        e::Severity::Hint => lsp::DiagnosticSeverity::HINT,
    }
}

pub fn diagnostic(d: e::Diagnostic) -> lsp::Diagnostic {
    let mut tags = Vec::new();
    if d.unnecessary {
        tags.push(lsp::DiagnosticTag::UNNECESSARY);
    }
    if d.deprecated {
        tags.push(lsp::DiagnosticTag::DEPRECATED);
    }
    lsp::Diagnostic {
        range: lsp_range(d.range),
        severity: Some(severity(d.severity)),
        code: d.code.map(lsp::NumberOrString::String),
        source: Some(d.source.unwrap_or_else(|| "php".into())),
        message: d.message,
        tags: (!tags.is_empty()).then_some(tags),
        ..lsp::Diagnostic::default()
    }
}

pub fn symbol_kind(k: e::SymbolKind) -> lsp::SymbolKind {
    use e::SymbolKind as K;
    match k {
        K::File => lsp::SymbolKind::FILE,
        K::Module => lsp::SymbolKind::MODULE,
        K::Namespace => lsp::SymbolKind::NAMESPACE,
        K::Package => lsp::SymbolKind::PACKAGE,
        K::Class => lsp::SymbolKind::CLASS,
        K::Interface => lsp::SymbolKind::INTERFACE,
        K::Struct => lsp::SymbolKind::STRUCT,
        K::Enum => lsp::SymbolKind::ENUM,
        K::EnumMember => lsp::SymbolKind::ENUM_MEMBER,
        // LSP has no trait kind; PHP tools show traits as classes.
        K::Trait => lsp::SymbolKind::CLASS,
        K::Function => lsp::SymbolKind::FUNCTION,
        K::Method => lsp::SymbolKind::METHOD,
        K::Constructor => lsp::SymbolKind::CONSTRUCTOR,
        K::Field => lsp::SymbolKind::FIELD,
        K::Property => lsp::SymbolKind::PROPERTY,
        K::Variable => lsp::SymbolKind::VARIABLE,
        K::Constant => lsp::SymbolKind::CONSTANT,
        K::TypeParameter => lsp::SymbolKind::TYPE_PARAMETER,
        K::TypeAlias => lsp::SymbolKind::CLASS,
        K::Macro => lsp::SymbolKind::FUNCTION,
        K::Event => lsp::SymbolKind::EVENT,
        K::Operator => lsp::SymbolKind::OPERATOR,
        K::Key => lsp::SymbolKind::KEY,
        K::Unknown => lsp::SymbolKind::NULL,
    }
}

#[allow(deprecated)]
pub fn document_symbol(item: e::OutlineItem) -> lsp::DocumentSymbol {
    lsp::DocumentSymbol {
        name: item.name,
        detail: item.detail,
        kind: symbol_kind(item.kind),
        tags: None,
        deprecated: None,
        range: lsp_range(item.range),
        selection_range: lsp_range(item.selection_range),
        children: Some(item.children.into_iter().map(document_symbol).collect()),
    }
}

pub fn completion_kind(k: php_engine::complete::CompletionKind) -> lsp::CompletionItemKind {
    use php_engine::complete::CompletionKind as K;
    match k {
        K::Class | K::Trait => lsp::CompletionItemKind::CLASS,
        K::Interface => lsp::CompletionItemKind::INTERFACE,
        K::Enum => lsp::CompletionItemKind::ENUM,
        K::Function => lsp::CompletionItemKind::FUNCTION,
        K::Method => lsp::CompletionItemKind::METHOD,
        K::Property => lsp::CompletionItemKind::PROPERTY,
        K::Constant => lsp::CompletionItemKind::CONSTANT,
        K::EnumCase => lsp::CompletionItemKind::ENUM_MEMBER,
        K::Variable | K::Parameter => lsp::CompletionItemKind::VARIABLE,
        K::Keyword => lsp::CompletionItemKind::KEYWORD,
        K::Value => lsp::CompletionItemKind::VALUE,
    }
}

/// What the client can apply.
#[derive(Clone, Copy, Debug, Default)]
pub struct EditSupport {
    /// `documentChanges` with versioned edits.
    pub document_changes: bool,
    /// Create, rename and delete operations in `documentChanges`.
    pub resource_operations: bool,
}

/// The engine's edit as the LSP's. Without `documentChanges` support only the
/// text edits are sent, as `changes`.
pub fn workspace_edit(edit: e::WorkspaceEdit, support: EditSupport) -> lsp::WorkspaceEdit {
    if support.document_changes {
        let mut ops = Vec::new();
        for file in edit.files {
            match file {
                e::FileEdit::Edit { path, edits, version } => {
                    let Some(uri) = uri_of(&path) else { continue };
                    ops.push(lsp::DocumentChangeOperation::Edit(lsp::TextDocumentEdit {
                        text_document: lsp::OptionalVersionedTextDocumentIdentifier { uri, version },
                        edits: edits.into_iter().map(|edit| lsp::OneOf::Left(text_edit(edit))).collect(),
                    }));
                }
                e::FileEdit::Create { path, overwrite } if support.resource_operations => {
                    let Some(uri) = uri_of(&path) else { continue };
                    ops.push(lsp::DocumentChangeOperation::Op(lsp::ResourceOp::Create(lsp::CreateFile {
                        uri,
                        options: Some(lsp::CreateFileOptions {
                            overwrite: Some(overwrite),
                            ignore_if_exists: Some(!overwrite),
                        }),
                        annotation_id: None,
                    })));
                }
                e::FileEdit::Rename { from, to } if support.resource_operations => {
                    let (Some(old_uri), Some(new_uri)) = (uri_of(&from), uri_of(&to)) else { continue };
                    ops.push(lsp::DocumentChangeOperation::Op(lsp::ResourceOp::Rename(lsp::RenameFile {
                        old_uri,
                        new_uri,
                        options: None,
                        annotation_id: None,
                    })));
                }
                e::FileEdit::Delete { path, recursive } if support.resource_operations => {
                    let Some(uri) = uri_of(&path) else { continue };
                    ops.push(lsp::DocumentChangeOperation::Op(lsp::ResourceOp::Delete(lsp::DeleteFile {
                        uri,
                        options: Some(lsp::DeleteFileOptions {
                            recursive: Some(recursive),
                            ignore_if_not_exists: Some(true),
                            annotation_id: None,
                        }),
                    })));
                }
                other => tracing::warn!("the client can't apply {other:?}; it is left out"),
            }
        }
        return lsp::WorkspaceEdit {
            changes: None,
            document_changes: Some(lsp::DocumentChanges::Operations(ops)),
            change_annotations: None,
        };
    }
    let mut changes: HashMap<Url, Vec<lsp::TextEdit>> = HashMap::new();
    for file in edit.files {
        match file {
            e::FileEdit::Edit { path, edits, .. } => {
                if let Some(uri) = uri_of(&path) {
                    changes.entry(uri).or_default().extend(edits.into_iter().map(text_edit));
                }
            }
            other => tracing::warn!("the client can't apply {other:?}; it is left out"),
        }
    }
    lsp::WorkspaceEdit { changes: Some(changes), document_changes: None, change_annotations: None }
}

/// Semantic tokens in the LSP's relative encoding. `tokens` are absolute
/// `(line, col, len, type, modifiers)`.
pub fn semantic_tokens(mut tokens: Vec<(u32, u32, u32, u32, u32)>) -> Vec<lsp::SemanticToken> {
    tokens.sort_by_key(|t| (t.0, t.1));
    let (mut line, mut col) = (0, 0);
    let mut out = Vec::with_capacity(tokens.len());
    for (l, c, length, token_type, token_modifiers_bitset) in tokens {
        let delta_line = l - line;
        let delta_start = if delta_line == 0 { c - col } else { c };
        out.push(lsp::SemanticToken { delta_line, delta_start, length, token_type, token_modifiers_bitset });
        (line, col) = (l, c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_encoded_relative_to_the_previous_one() {
        let encoded = semantic_tokens(vec![(2, 4, 3, 1, 0), (0, 6, 5, 0, 1), (2, 10, 2, 2, 0)]);
        let flat: Vec<_> = encoded
            .iter()
            .map(|t| (t.delta_line, t.delta_start, t.length, t.token_type, t.token_modifiers_bitset))
            .collect();
        assert_eq!(flat, [(0, 6, 5, 0, 1), (2, 4, 3, 1, 0), (0, 6, 2, 2, 0)]);
    }

    #[test]
    fn edits_fall_back_to_changes_without_document_changes() {
        let dir = std::env::temp_dir();
        let a = dir.join("a.php");
        let edit = e::WorkspaceEdit {
            files: vec![
                e::FileEdit::Edit {
                    path: a.clone(),
                    edits: vec![e::TextEdit { range: e::Range::default(), new_text: "x".into() }],
                    version: None,
                },
                e::FileEdit::Rename { from: a.clone(), to: dir.join("b.php") },
            ],
        };
        let plain = workspace_edit(edit.clone(), EditSupport::default());
        assert_eq!(plain.changes.unwrap()[&uri_of(&a).unwrap()].len(), 1);
        let full = workspace_edit(edit, EditSupport { document_changes: true, resource_operations: true });
        let Some(lsp::DocumentChanges::Operations(ops)) = full.document_changes else { panic!() };
        assert_eq!(ops.len(), 2);
    }
}
