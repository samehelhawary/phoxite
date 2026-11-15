//! Applying an edit the server answered: `{ edit, created }`, where `edit` is an LSP
//! `WorkspaceEdit` and `created` the URIs of the files it creates.
//!
//! The generators create files and edit a few existing ones (a migration and
//! `$fillable`, a translation file), so this handles file creation and text edits;
//! renames and deletions are refused instead of being half applied.

use openidea_plugin::{Error, Position, Project, Range, TextEdit};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
pub struct Applied {
    edit: Value,
    #[serde(default)]
    created: Vec<String>,
}

#[derive(Deserialize)]
struct LspPosition {
    line: u32,
    character: u32,
}

#[derive(Deserialize)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LspEdit {
    range: LspRange,
    new_text: String,
}

/// What happens to one file: it is created, and its text is edited.
struct FileChange {
    path: String,
    create: bool,
    edits: Vec<TextEdit>,
}

/// The path of a `file:` URI: `file:///C:/a%20b/c.php` is `C:/a b/c.php`.
pub fn uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    // An authority (`file://host/share`) names a network path.
    let (host, path) = match rest.strip_prefix('/') {
        Some(path) => ("", path),
        None => rest.split_once('/')?,
    };
    let mut bytes = Vec::with_capacity(path.len());
    let raw = path.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (raw[i], raw.get(i + 1).copied().and_then(hex), raw.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(high), Some(low)) => {
                bytes.push((high * 16 + low) as u8);
                i += 3;
            }
            (b, _, _) => {
                bytes.push(b);
                i += 1;
            }
        }
    }
    let path = String::from_utf8(bytes).ok()?;
    let drive = path.as_bytes().get(1) == Some(&b':') && path.as_bytes()[0].is_ascii_alphabetic();
    Some(match (host.is_empty(), drive) {
        (false, _) => format!("//{host}/{path}"),
        (true, true) => path,
        (true, false) => format!("/{path}"),
    })
}

fn text_edits(edits: &Value) -> Result<Vec<TextEdit>, Error> {
    let edits: Vec<LspEdit> = serde_json::from_value(edits.clone()).map_err(malformed)?;
    Ok(edits
        .into_iter()
        .map(|e| TextEdit {
            range: Range::new(
                Position::new(e.range.start.line, e.range.start.character),
                Position::new(e.range.end.line, e.range.end.character),
            ),
            text: e.new_text,
        })
        .collect())
}

fn malformed(e: impl std::fmt::Display) -> Error {
    Error::Failed(format!("The server's edit can't be read: {e}"))
}

fn path_of(uri: &Value) -> Result<String, Error> {
    uri.as_str().and_then(uri_to_path).ok_or_else(|| malformed("a URI that is not a file"))
}

/// The files of a `WorkspaceEdit`, in the order the edit names them.
fn changes(edit: &Value) -> Result<Vec<FileChange>, Error> {
    let mut files: Vec<FileChange> = Vec::new();
    fn entry(files: &mut Vec<FileChange>, path: String) -> &mut FileChange {
        let at = files.iter().position(|f| f.path == path).unwrap_or_else(|| {
            files.push(FileChange { path, create: false, edits: Vec::new() });
            files.len() - 1
        });
        &mut files[at]
    }
    if let Some(operations) = edit.get("documentChanges").and_then(Value::as_array) {
        for operation in operations {
            match operation.get("kind").and_then(Value::as_str) {
                Some("create") => entry(&mut files, path_of(&operation["uri"])?).create = true,
                Some(kind) => {
                    return Err(Error::Invalid(format!("The edit wants to {kind} a file, which is not supported")));
                }
                None => {
                    let file = entry(&mut files, path_of(&operation["textDocument"]["uri"])?);
                    file.edits.extend(text_edits(&operation["edits"])?);
                }
            }
        }
    } else if let Some(by_uri) = edit.get("changes").and_then(Value::as_object) {
        for (uri, edits) in by_uri {
            let file = entry(&mut files, path_of(&Value::String(uri.clone()))?);
            file.edits.extend(text_edits(edits)?);
        }
    }
    Ok(files)
}

/// The byte offset of an LSP position (UTF-16 columns) in `text`, clamped to it.
fn offset(text: &str, at: Position) -> usize {
    let mut start = 0;
    for _ in 0..at.line {
        match text[start..].find('\n') {
            Some(end) => start += end + 1,
            None => return text.len(),
        }
    }
    let line = text[start..].split('\n').next().unwrap_or_default();
    let line = line.strip_suffix('\r').unwrap_or(line);
    let mut units = 0;
    for (index, c) in line.char_indices() {
        if units >= at.character {
            return start + index;
        }
        units += c.len_utf16() as u32;
    }
    start + line.len()
}

/// `text` with `edits` applied; the ranges are those of the text before any of them.
pub fn apply_text_edits(text: &str, edits: &[TextEdit]) -> String {
    let mut spans: Vec<(usize, usize, usize)> = edits
        .iter()
        .enumerate()
        .map(|(index, e)| {
            let (start, end) = (offset(text, e.range.start), offset(text, e.range.end));
            (start, end.max(start), index)
        })
        .collect();
    // From the end of the text backwards; edits at one place keep their order.
    spans.sort_by(|a, b| b.0.cmp(&a.0).then(b.2.cmp(&a.2)));
    let mut out = text.to_owned();
    for (start, end, index) in spans {
        out.replace_range(start..end, &edits[index].text);
    }
    out
}

/// Applies the edit to the project's files and answers the paths of the files it
/// created. A file open with unsaved changes is edited in its editor instead.
pub fn apply(project: &Project, applied: &Applied) -> Result<Vec<String>, Error> {
    let files = changes(&applied.edit)?;
    if files.is_empty() {
        return Err(Error::Failed("The server answered no edit".into()));
    }
    for file in &files {
        let before = match project.read_file(&file.path) {
            Ok(text) => text,
            Err(Error::NotFound(_)) => String::new(),
            Err(e) => return Err(e),
        };
        if file.edits.is_empty() && !(file.create && before.is_empty()) {
            continue;
        }
        let after = apply_text_edits(&before, &file.edits);
        match project.write_file(&file.path, &after) {
            // The file has unsaved changes in an editor: edit it there.
            Err(Error::Invalid(_)) if !file.create => {
                let document = project.document(&file.path)?;
                project.apply_document_edits(&file.path, document.version, &file.edits)?;
            }
            result => result?,
        }
    }
    Ok(applied.created.iter().filter_map(|uri| uri_to_path(uri)).collect())
}

/// Applies the edit and shows the first file it created.
pub fn apply_and_open(project: &Project, applied: &Applied) -> Result<Vec<String>, Error> {
    let created = apply(project, applied)?;
    if let Some(first) = created.first() {
        openidea_plugin::host::open_location(first, 0, 0)?;
    }
    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(start: (u32, u32), end: (u32, u32), text: &str) -> TextEdit {
        TextEdit { range: Range::new(Position::new(start.0, start.1), Position::new(end.0, end.1)), text: text.into() }
    }

    #[test]
    fn file_uris_become_paths() {
        assert_eq!(uri_to_path("file:///C:/a%20b/Post.php").as_deref(), Some("C:/a b/Post.php"));
        assert_eq!(uri_to_path("file:///home/me/app/Post.php").as_deref(), Some("/home/me/app/Post.php"));
        assert_eq!(uri_to_path("file://server/share/Post.php").as_deref(), Some("//server/share/Post.php"));
        assert_eq!(uri_to_path("file:///home/caf%C3%A9.php").as_deref(), Some("/home/café.php"));
        assert_eq!(uri_to_path("untitled:Post.php"), None);
    }

    #[test]
    fn text_edits_use_the_original_positions_and_utf16_columns() {
        let text = "<?php\n$a = '😀';\n$b = 2;\n";
        let edits = [
            edit((2, 5), (2, 6), "3"),
            // After the emoji, which is two UTF-16 units.
            edit((1, 8), (1, 8), "!"),
            edit((0, 5), (0, 5), "\n// one"),
            edit((0, 5), (0, 5), "\n// two"),
        ];
        assert_eq!(apply_text_edits(text, &edits), "<?php\n// one\n// two\n$a = '😀!';\n$b = 3;\n");
        // Positions past the end are the end, as an insert into an empty new file is.
        assert_eq!(apply_text_edits("", &[edit((0, 0), (0, 0), "<?php\n")]), "<?php\n");
        assert_eq!(apply_text_edits("a\r\nb", &[edit((0, 9), (1, 0), "-")]), "a-b");
    }

    #[test]
    fn document_changes_and_plain_changes_name_the_same_files() {
        let range = serde_json::json!({ "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } });
        let operations = serde_json::json!({ "documentChanges": [
            { "kind": "create", "uri": "file:///p/app/Post.php", "options": { "ignoreIfExists": true } },
            { "textDocument": { "uri": "file:///p/app/Post.php", "version": null },
              "edits": [{ "range": range, "newText": "<?php\n" }] },
            { "textDocument": { "uri": "file:///p/app/User.php", "version": 3 },
              "edits": [{ "range": range, "newText": "x" }] },
        ] });
        let files = changes(&operations).unwrap();
        let seen: Vec<_> = files.iter().map(|f| (f.path.as_str(), f.create, f.edits.len())).collect();
        assert_eq!(seen, [("/p/app/Post.php", true, 1), ("/p/app/User.php", false, 1)]);

        let plain =
            serde_json::json!({ "changes": { "file:///p/app/User.php": [{ "range": range, "newText": "x" }] } });
        assert_eq!(changes(&plain).unwrap()[0].path, "/p/app/User.php");

        let rename = serde_json::json!({ "documentChanges": [
            { "kind": "rename", "oldUri": "file:///p/a.php", "newUri": "file:///p/b.php" },
        ] });
        assert!(matches!(changes(&rename), Err(Error::Invalid(m)) if m.contains("rename")));
    }
}
