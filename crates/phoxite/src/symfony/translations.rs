//! The translation keys of a Symfony project: what the YAML and XLIFF files
//! of its `translations/` folder define, as Symfony's documentation on
//! translations names them (`messages.en.yaml` is the domain `messages` in
//! the locale `en`; `messages+intl-icu.en.yaml` is the same domain).
//!
//! In YAML a key is a top-level key, or the keys of nested mappings joined
//! by dots (`form: { title: … }` is `form.title`). In XLIFF (`.xlf`,
//! `.xliff`; versions 1.2 and 2.0) a key is a unit's `resname` (or `name`),
//! else the text of its `<source>`, and the translation is its `<target>`.
//! In JSON a key is a key of the object, or the keys of nested objects
//! joined by dots. PHP catalogues aren't read.

use std::path::{Path, PathBuf};

use crate::base::Range;
use crate::base::yaml::{self, Node};

/// The most translation files one load reads.
const MAX_FILES: usize = 256;

/// One key of one catalogue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TranslationKey {
    pub key: String,
    /// `messages`, `validators`, …
    pub domain: String,
    pub locale: String,
    /// The translation, when it is a scalar.
    pub value: Option<String>,
    pub file: PathBuf,
    /// The key as written (its last part, for a nested key).
    pub range: Range,
}

/// The keys of the YAML catalogues below `root/translations`, in file then
/// source order.
pub fn load(root: &Path) -> Vec<TranslationKey> {
    let Ok(entries) = std::fs::read_dir(root.join("translations")) else { return Vec::new() };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(path.extension().and_then(|e| e.to_str()), Some("yaml" | "yml" | "xlf" | "xliff" | "json"))
        })
        .collect();
    files.sort();
    let mut out = Vec::new();
    for file in files.into_iter().take(MAX_FILES) {
        let Some((domain, locale)) = catalogue(&file) else { continue };
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let extension = file.extension().and_then(|e| e.to_str());
        if matches!(extension, Some("xlf" | "xliff")) {
            add_xliff(&text, &domain, &locale, &file, &mut out);
        } else if extension == Some("json") {
            add_json(&text, &domain, &locale, &file, &mut out);
        } else {
            add(&yaml::parse(&text), "", &domain, &locale, &file, &mut out);
        }
    }
    out
}

/// The keys of a JSON catalogue: the keys of its object, those of nested
/// objects joined by dots. The text is walked as written, so that each key
/// has its place; what isn't an object of strings and objects is skipped.
fn add_json(text: &str, domain: &str, locale: &str, file: &Path, out: &mut Vec<TranslationKey>) {
    let index = crate::syntax::LineIndex::new(text);
    let bytes = text.as_bytes();
    // The end of the string starting at the quote `at` (the byte of its closing quote).
    let string_end = |at: usize| {
        let mut end = at + 1;
        while end < bytes.len() && bytes[end] != b'"' {
            if bytes[end] == b'\\' {
                end += 1;
            }
            end += 1;
        }
        end.min(bytes.len())
    };
    let unquoted = |from: usize, to: usize| {
        serde_json::from_str::<String>(&text[from..=to.min(text.len() - 1)])
            .unwrap_or_else(|_| text[from + 1..to].to_owned())
    };
    // The keys of the objects around, and for each object whether a key comes next.
    let mut path: Vec<String> = Vec::new();
    let mut objects: Vec<bool> = Vec::new();
    // The key read last, waiting for its value: its text and the bytes of its content.
    let mut pending: Option<(String, usize, usize)> = None;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                let end = string_end(at);
                let expects_key = objects.last().copied().unwrap_or(false);
                if expects_key {
                    pending = Some((unquoted(at, end), at + 1, end));
                    if let Some(last) = objects.last_mut() {
                        *last = false;
                    }
                } else if let Some((key, from, to)) = pending.take() {
                    let full = path.iter().chain(std::iter::once(&key)).cloned().collect::<Vec<_>>().join(".");
                    out.push(TranslationKey {
                        key: full,
                        domain: domain.to_owned(),
                        locale: locale.to_owned(),
                        value: Some(unquoted(at, end)),
                        file: file.to_path_buf(),
                        range: index.range(from as u32, to as u32),
                    });
                }
                at = end;
            }
            b'{' => {
                if let Some((key, ..)) = pending.take() {
                    path.push(key);
                } else if !objects.is_empty() {
                    // An object inside an array: its keys are no catalogue's.
                    path.push(String::new());
                }
                objects.push(true);
            }
            b'}' => {
                objects.pop();
                if !objects.is_empty() {
                    path.pop();
                }
            }
            b',' => {
                pending = None;
                if let Some(last) = objects.last_mut() {
                    *last = true;
                }
            }
            b'[' => {
                // A list has no key: skip to its end.
                let mut depth = 0usize;
                while at < bytes.len() {
                    match bytes[at] {
                        b'"' => at = string_end(at),
                        b'[' => depth += 1,
                        b']' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    at += 1;
                }
                pending = None;
            }
            _ => {}
        }
        at += 1;
    }
}

/// The text of the XML entities XLIFF texts use.
fn unescaped(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// The content of the first `<tag …>…</tag>` of `text`, with the byte it starts at.
fn element<'t>(text: &'t str, tag: &str) -> Option<(&'t str, usize)> {
    let open = text.find(&format!("<{tag}"))?;
    let start = open + text[open..].find('>')? + 1;
    // `<target/>`: nothing in it.
    if text[..start].ends_with("/>") {
        return Some(("", start));
    }
    let end = start + text[start..].find(&format!("</{tag}>"))?;
    Some((&text[start..end], start))
}

/// The value of the attribute `name` of the opening tag `tag` (its text
/// up to `>`), with the byte it starts at.
fn attribute<'t>(tag: &'t str, name: &str) -> Option<(&'t str, usize)> {
    let mut from = 0;
    while let Some(found) = tag[from..].find(name) {
        let at = from + found;
        from = at + name.len();
        // A whole attribute name, followed by `="`.
        let whole = tag[..at].ends_with(char::is_whitespace);
        let Some(rest) = tag[from..].strip_prefix("=\"") else { continue };
        if whole {
            let end = rest.find('"')?;
            return Some((&rest[..end], from + 2));
        }
    }
    None
}

/// The keys of an XLIFF catalogue: one per `<trans-unit>` (1.2) or `<unit>` (2.0).
fn add_xliff(text: &str, domain: &str, locale: &str, file: &Path, out: &mut Vec<TranslationKey>) {
    let index = crate::syntax::LineIndex::new(text);
    for (unit, close) in [("<trans-unit", "</trans-unit>"), ("<unit", "</unit>")] {
        let mut from = 0;
        while let Some(found) = text[from..].find(unit) {
            let start = from + found;
            let end = text[start..].find(close).map_or(text.len(), |at| start + at);
            from = end;
            // `<unit` also starts `<units>`: the tag's name has to end there.
            if !text[start + unit.len()..].starts_with(|c: char| c.is_whitespace() || c == '>') {
                from = start + unit.len();
                continue;
            }
            let body = &text[start..end];
            let tag_end = body.find('>').unwrap_or(body.len());
            let named = attribute(&body[..tag_end], "resname").or_else(|| attribute(&body[..tag_end], "name"));
            let Some((key, at)) = named.or_else(|| element(body, "source")) else { continue };
            if key.trim().is_empty() {
                continue;
            }
            let value = element(body, "target").map(|(target, _)| unescaped(target.trim()));
            let (from_byte, to_byte) = ((start + at) as u32, (start + at + key.len()) as u32);
            out.push(TranslationKey {
                key: unescaped(key.trim()),
                domain: domain.to_owned(),
                locale: locale.to_owned(),
                value: value.filter(|value| !value.is_empty()),
                file: file.to_path_buf(),
                range: index.range(from_byte, to_byte),
            });
        }
    }
}

/// The domain and the locale a catalogue's file name says (`messages.en.yaml`).
fn catalogue(file: &Path) -> Option<(String, String)> {
    let name = file.file_stem()?.to_str()?;
    let (domain, locale) = name.rsplit_once('.')?;
    let domain = domain.strip_suffix("+intl-icu").unwrap_or(domain);
    (!domain.is_empty() && !locale.is_empty()).then(|| (domain.to_owned(), locale.to_owned()))
}

fn add(node: &Node, prefix: &str, domain: &str, locale: &str, file: &Path, out: &mut Vec<TranslationKey>) {
    for entry in node.entries() {
        let key = if prefix.is_empty() { entry.key.clone() } else { format!("{prefix}.{}", entry.key) };
        match &entry.value {
            Node::Map(_) => add(&entry.value, &key, domain, locale, file, out),
            value => out.push(TranslationKey {
                key,
                domain: domain.to_owned(),
                locale: locale.to_owned(),
                value: value.as_str().map(str::to_owned),
                file: file.to_path_buf(),
                range: entry.range,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_of_the_yaml_catalogues_with_their_domain_and_locale() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("translations");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("messages.en.yaml"), "welcome: Hello\nform:\n    title: Title\n    hint: ~\n")
            .unwrap();
        std::fs::write(folder.join("messages+intl-icu.fr.yml"), "welcome: Bonjour\n").unwrap();
        std::fs::write(folder.join("validators.en.yaml"), "too.short: Too short\n").unwrap();
        std::fs::write(folder.join("messages.en.xlf"), "<xliff/>").unwrap();
        std::fs::write(folder.join("notes.yaml"), "x: y\n").unwrap();
        let keys = load(dir.path());
        let found: Vec<(&str, &str, &str, Option<&str>)> =
            keys.iter().map(|k| (k.key.as_str(), k.domain.as_str(), k.locale.as_str(), k.value.as_deref())).collect();
        assert_eq!(
            found,
            [
                ("welcome", "messages", "fr", Some("Bonjour")),
                ("welcome", "messages", "en", Some("Hello")),
                ("form.title", "messages", "en", Some("Title")),
                ("form.hint", "messages", "en", None),
                ("too.short", "validators", "en", Some("Too short")),
            ]
        );
        // A nested key's range is where its last part is written.
        let title = keys.iter().find(|k| k.key == "form.title").unwrap();
        assert_eq!((title.range.start.line, title.range.start.col), (2, 4));
        assert_eq!(load(&dir.path().join("nowhere")), []);
    }

    #[test]
    fn keys_of_json_catalogues() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("translations");
        std::fs::create_dir_all(&folder).unwrap();
        let json = "{\n  \"welcome\": \"Hello \\\"you\\\"\",\n  \"form\": {\n    \"title\": \"Title\",\n    \"tags\": [\"a\", {\"x\": \"y\"}],\n    \"hint\": \"Hint\"\n  },\n  \"count\": 3,\n  \"bye\": \"Bye\"\n}\n";
        std::fs::write(folder.join("messages.en.json"), json).unwrap();
        let keys = load(dir.path());
        let found: Vec<(&str, Option<&str>, u32, u32)> =
            keys.iter().map(|k| (k.key.as_str(), k.value.as_deref(), k.range.start.line, k.range.start.col)).collect();
        assert_eq!(
            found,
            [
                ("welcome", Some("Hello \"you\""), 1, 3),
                ("form.title", Some("Title"), 3, 5),
                ("form.hint", Some("Hint"), 5, 5),
                ("bye", Some("Bye"), 8, 3),
            ]
        );
    }

    #[test]
    fn keys_of_xliff_catalogues() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("translations");
        std::fs::create_dir_all(&folder).unwrap();
        let old = "<?xml version=\"1.0\"?>\n<xliff version=\"1.2\">\n  <file source-language=\"en\" target-language=\"fr\">\n    <body>\n      <trans-unit id=\"1\" resname=\"form.title\">\n        <source>form.title</source>\n        <target>Titre &amp; plus</target>\n      </trans-unit>\n      <trans-unit id=\"2\">\n        <source>Hello</source>\n        <target/>\n      </trans-unit>\n    </body>\n  </file>\n</xliff>\n";
        std::fs::write(folder.join("messages.fr.xlf"), old).unwrap();
        let new = "<xliff version=\"2.0\" srcLang=\"en\" trgLang=\"de\">\n  <file id=\"f\">\n    <unit id=\"u1\" name=\"welcome\">\n      <segment><source>welcome</source><target>Willkommen</target></segment>\n    </unit>\n  </file>\n</xliff>\n";
        std::fs::write(folder.join("messages.de.xliff"), new).unwrap();
        let keys = load(dir.path());
        let found: Vec<(&str, &str, Option<&str>, u32)> =
            keys.iter().map(|k| (k.key.as_str(), k.locale.as_str(), k.value.as_deref(), k.range.start.line)).collect();
        assert_eq!(
            found,
            [
                ("welcome", "de", Some("Willkommen"), 2),
                ("form.title", "fr", Some("Titre & plus"), 4),
                ("Hello", "fr", None, 9),
            ]
        );
    }
}
