//! What a WordPress project says outside PHP: the blocks its `block.json`
//! files declare ([`blocks`]), and the names its scripts write
//! ([`script_usages`]): a block's, the action of an AJAX request, the path
//! of a REST route.
//!
//! The engine doesn't parse JavaScript. A script is searched as text for a
//! name inside quotes, which is enough to list a usage and to rename it;
//! nothing is completed or gone to from a script.
//!
//! WordPress's own folders, `node_modules`, `vendor`, minified files and
//! large ones aren't read.

use std::path::{Path, PathBuf};

use crate::base::Range;
use crate::syntax::LineIndex;

/// How deep below the project's folder files are looked for.
const MAX_DEPTH: usize = 12;

/// The most files one search reads.
const MAX_FILES: usize = 5000;

/// The largest file read.
const MAX_BYTES: u64 = 512 * 1024;

const SCRIPTS: &[&str] = &["js", "jsx", "mjs", "ts", "tsx"];

/// A block as a `block.json` declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockJson {
    pub name: String,
    pub path: PathBuf,
    /// The name's text, inside its quotes.
    pub range: Range,
}

/// Calls `found` with the files below `folder` that `wanted` says are
/// read, until [`MAX_FILES`] of them were.
fn walk(folder: &Path, depth: usize, left: &mut usize, wanted: &dyn Fn(&Path) -> bool, found: &mut dyn FnMut(PathBuf)) {
    let Ok(entries) = std::fs::read_dir(folder) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if *left == 0 {
            return;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if path.is_dir() {
            let skipped =
                name.starts_with('.') || matches!(name, "node_modules" | "vendor" | "wp-admin" | "wp-includes");
            if depth < MAX_DEPTH && !skipped {
                walk(&path, depth + 1, left, wanted, found);
            }
        } else if wanted(&path) && path.metadata().is_ok_and(|file| file.len() <= MAX_BYTES) {
            *left -= 1;
            found(path);
        }
    }
}

fn is_script(path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    let script = path.extension().and_then(|ext| ext.to_str()).is_some_and(|ext| SCRIPTS.contains(&ext));
    script && !name.contains(".min.")
}

/// Whether `path` is a `block.json`.
pub fn is_block_json(path: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "block.json")
}

/// The blocks the `block.json` files below `root` declare, in path order.
pub fn blocks(root: &Path) -> Vec<BlockJson> {
    let mut out = Vec::new();
    let mut left = MAX_FILES;
    walk(root, 0, &mut left, &is_block_json, &mut |path| {
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let Ok(serde_json::Value::Object(block)) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(name) = block.get("name").and_then(|name| name.as_str()) else { return };
        // The first `"name": "…"` saying it: the block's own comes before those of its attributes.
        let quoted = format!("\"{name}\"");
        let mut from = 0;
        while let Some(at) = text[from..].find("\"name\"") {
            let key_end = from + at + "\"name\"".len();
            let rest = text[key_end..].trim_start();
            if let Some(value) = rest.strip_prefix(':').map(str::trim_start)
                && value.starts_with(&quoted)
            {
                let start = text.len() - value.len() + 1;
                let range = LineIndex::new(&text).range(start as u32, (start + name.len()) as u32);
                out.push(BlockJson { name: name.to_owned(), path: path.clone(), range });
                return;
            }
            from = key_end;
        }
    });
    out
}

/// How a script writes a name.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// A string that is the name: `registerBlockType('shop/price', …)`.
    Name,
    /// The action of an AJAX request: a string that is the name on a line
    /// saying `action` before it (`action: 'shop_add'`,
    /// `data.append('action', 'shop_add')`), or `action=shop_add` in a URL.
    AjaxAction,
    /// The path of a REST route: a string that is the path, a `/` before it
    /// or not, a query after it or not (`apiFetch({ path:
    /// '/shop/v1/books?per_page=5' })`), or the end of a longer URL.
    RestPath,
    /// The path of a REST route or the start of a longer one:
    /// `'/shop/v1/books/12'` for `shop/v1/books`.
    RestUnder,
}

fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// The bytes of `text` where `name` is written as `how`.
pub fn written(text: &str, name: &str, how: Written) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let quote = |byte: Option<&u8>| matches!(byte, Some(b'\'' | b'"' | b'`'));
    let mut out = Vec::new();
    let mut from = 0;
    while !name.is_empty()
        && let Some(at) = text[from..].find(name)
    {
        let start = from + at;
        let end = start + name.len();
        from = end;
        let before = start.checked_sub(1).and_then(|at| bytes.get(at));
        let after = bytes.get(end);
        let line = text[..start].rfind('\n').map_or(0, |at| at + 1);
        let found = match how {
            Written::Name => quote(before) && before == after,
            Written::AjaxAction => {
                let string = quote(before) && before == after && text[line..start - 1].contains("action");
                let query = text[..start].ends_with("action=") && !after.is_some_and(|byte| word(*byte));
                string || query
            }
            Written::RestPath => {
                let opened = quote(before) || (before == Some(&b'/') && text[line..start].contains(['\'', '"', '`']));
                let closed = quote(after) || matches!(after, Some(b'?' | b'#')) || {
                    // A slash and then the string's end.
                    after == Some(&b'/') && quote(bytes.get(end + 1))
                };
                opened && closed
            }
            Written::RestUnder => {
                let opened = quote(before) || (before == Some(&b'/') && text[line..start].contains(['\'', '"', '`']));
                opened && (quote(after) || matches!(after, Some(b'?' | b'#' | b'/')))
            }
        };
        if found {
            out.push((start, end));
        }
    }
    out
}

/// Where the scripts below `root` write `name` as `how`; a block's name
/// ([`Written::Name`]) in the `block.json` files too (`"parent"`,
/// `"allowedBlocks"`, and the block's own).
pub fn script_usages(root: &Path, name: &str, how: Written) -> Vec<(PathBuf, Range)> {
    let mut out = Vec::new();
    let mut left = MAX_FILES;
    let read = |path: &Path| is_script(path) || (how == Written::Name && is_block_json(path));
    walk(root, 0, &mut left, &read, &mut |path| {
        let Ok(text) = std::fs::read_to_string(&path) else { return };
        let found = written(&text, name, how);
        if found.is_empty() {
            return;
        }
        let lines = LineIndex::new(&text);
        out.extend(found.into_iter().map(|(start, end)| (path.clone(), lines.range(start as u32, end as u32))));
    });
    out
}

#[cfg(test)]
mod tests {
    use super::{Written, written};

    fn found(text: &str, name: &str, how: Written) -> Vec<String> {
        written(text, name, how).into_iter().map(|(start, _)| text[..start].to_owned()).collect()
    }

    #[test]
    fn names_are_found_in_a_scripts_text() {
        let script = "registerBlockType('shop/price', {});\nconst other = \"shop/price-list\";\nuse(`shop/price`);\n";
        assert_eq!(written(script, "shop/price", Written::Name).len(), 2);

        let script = "jQuery.post(url, { action: 'shop_add', id });\nconst name = 'shop_add';\ndata.append('action', \"shop_add\");\nfetch(url + '?action=shop_add&id=1');\nfetch('?action=shop_add_more');\n";
        let lines: Vec<usize> =
            found(script, "shop_add", Written::AjaxAction).iter().map(|before| before.matches('\n').count()).collect();
        assert_eq!(lines, [0, 2, 3]);

        let script = "apiFetch({ path: '/shop/v1/books' });\napiFetch({ path: 'shop/v1/books?per_page=5' });\nfetch(root + 'wp-json/shop/v1/books/');\nfetch('/shop/v1/books/12');\nfetch('/shop/v1/bookshelf');\n";
        let lines: Vec<usize> = found(script, "shop/v1/books", Written::RestPath)
            .iter()
            .map(|before| before.matches('\n').count())
            .collect();
        assert_eq!(lines, [0, 1, 2]);
    }
}
