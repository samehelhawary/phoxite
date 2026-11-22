//! A small XML reader: the elements of a document, each with its
//! attributes, its children and its text, and where each name and value is
//! written. It is what the engine needs of a configuration file written in
//! XML (Magento's `di.xml`, `events.xml`, layout files): names to go to.
//!
//! It never fails. A file being edited reads as far as it makes sense: an
//! element that is never closed ends with the document, a closing tag that
//! closes nothing is skipped. Comments, processing instructions and the
//! document type are skipped; a CDATA section is text. Entities are the five
//! XML has and character references; namespaces are not resolved (`xsi:type`
//! is an attribute of that name).

use crate::base::Range;
use crate::syntax::LineIndex;

/// An attribute of an element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub value: String,
    /// Where the value is written, without its quotes.
    pub range: Range,
}

/// An element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    /// Where the name is written in the opening tag.
    pub range: Range,
    pub attributes: Vec<Attribute>,
    pub children: Vec<Element>,
    /// The text directly inside it, trimmed.
    pub text: String,
    /// Where that text is written.
    pub text_range: Range,
}

impl Element {
    /// The attribute named `name`.
    pub fn attribute(&self, name: &str) -> Option<&Attribute> {
        self.attributes.iter().find(|attribute| attribute.name == name)
    }

    /// The value of the attribute named `name`.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attribute(name).map(|attribute| attribute.value.as_str())
    }

    /// The children named `name`.
    pub fn named<'e>(&'e self, name: &'e str) -> impl Iterator<Item = &'e Element> + 'e {
        self.children.iter().filter(move |child| child.name == name)
    }

    /// The first child named `name`.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|child| child.name == name)
    }

    /// The element and every element below it, in the order they are
    /// written.
    pub fn descendants(&self) -> Vec<&Element> {
        fn walk<'e>(element: &'e Element, out: &mut Vec<&'e Element>) {
            out.push(element);
            for child in &element.children {
                walk(child, out);
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }
}

/// The text of the entities and character references of `text`.
fn decode(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest.find(';').filter(|end| *end <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let known = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => {
                let code = match name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok(),
                    None => name.strip_prefix('#').and_then(|decimal| decimal.parse().ok()),
                };
                code.and_then(char::from_u32)
            }
        };
        match known {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// An element being read: its direct text and where that text starts and ends.
struct Open {
    element: Element,
    text: String,
    text_at: Option<(usize, usize)>,
}

struct Reader<'t> {
    text: &'t str,
    lines: LineIndex,
    at: usize,
    open: Vec<Open>,
    roots: Vec<Element>,
}

fn is_name(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')
}

impl Reader<'_> {
    fn range(&self, start: usize, end: usize) -> Range {
        self.lines.range(start as u32, end as u32)
    }

    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    /// Skips to after `end`, or to the end of the document.
    fn skip_past(&mut self, end: &str) {
        self.at = self.rest().find(end).map_or(self.text.len(), |found| self.at + found + end.len());
    }

    fn skip_space(&mut self) {
        let rest = self.rest();
        self.at += rest.len() - rest.trim_start().len();
    }

    fn name(&mut self) -> (String, usize, usize) {
        let start = self.at;
        let rest = &self.text[start..];
        let length = rest.find(|c: char| !is_name(c)).unwrap_or(rest.len());
        let name = rest[..length].to_owned();
        self.at += length;
        (name, start, self.at)
    }

    /// Text between two tags: the innermost open element's.
    fn add_text(&mut self, start: usize, end: usize, decoded: bool) {
        let Some(open) = self.open.last_mut() else { return };
        let raw = &self.text[start..end];
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let from = start + (raw.len() - raw.trim_start().len());
        let to = from + trimmed.len();
        open.text.push_str(&if decoded { decode(trimmed) } else { trimmed.to_owned() });
        open.text_at = Some(match open.text_at {
            Some((first, _)) => (first, to),
            None => (from, to),
        });
    }

    fn close(&mut self) {
        let Some(open) = self.open.pop() else { return };
        let mut element = open.element;
        element.text = open.text;
        if let Some((start, end)) = open.text_at {
            element.text_range = self.range(start, end);
        }
        match self.open.last_mut() {
            Some(parent) => parent.element.children.push(element),
            None => self.roots.push(element),
        }
    }

    fn read(&mut self) {
        while self.at < self.text.len() {
            let Some(found) = self.rest().find('<') else {
                self.add_text(self.at, self.text.len(), true);
                break;
            };
            let tag = self.at + found;
            self.add_text(self.at, tag, true);
            self.at = tag;
            let rest = self.rest();
            if rest.starts_with("<!--") {
                self.skip_past("-->");
            } else if rest.starts_with("<![CDATA[") {
                let start = self.at + 9;
                let end = self.text[start..].find("]]>").map_or(self.text.len(), |end| start + end);
                self.add_text(start, end, false);
                self.at = (end + 3).min(self.text.len());
            } else if rest.starts_with("<?") {
                self.skip_past("?>");
            } else if rest.starts_with("<!") {
                self.skip_past(">");
            } else if rest.starts_with("</") {
                self.at += 2;
                let (name, _, _) = self.name();
                self.skip_past(">");
                // The innermost open element of that name, and what is open inside it.
                if let Some(at) = self.open.iter().rposition(|open| open.element.name == name) {
                    while self.open.len() > at {
                        self.close();
                    }
                }
            } else {
                self.at += 1;
                self.open_tag();
            }
        }
        while !self.open.is_empty() {
            self.close();
        }
    }

    /// After `<`: a name, attributes, and `>` or `/>`.
    fn open_tag(&mut self) {
        let (name, start, end) = self.name();
        if name.is_empty() {
            // A `<` that starts no tag: text.
            return;
        }
        let mut element = Element {
            name,
            range: self.range(start, end),
            attributes: Vec::new(),
            children: Vec::new(),
            text: String::new(),
            text_range: Range::default(),
        };
        loop {
            self.skip_space();
            let rest = self.rest();
            if rest.is_empty() {
                break;
            }
            if rest.starts_with("/>") {
                self.at += 2;
                self.open.push(Open { element, text: String::new(), text_at: None });
                self.close();
                return;
            }
            if rest.starts_with('>') {
                self.at += 1;
                break;
            }
            // Another tag starts before this one ends: a tag being typed.
            if rest.starts_with('<') {
                break;
            }
            let (name, _, _) = self.name();
            if name.is_empty() {
                self.at += self.rest().chars().next().map_or(1, char::len_utf8);
                continue;
            }
            self.skip_space();
            if !self.rest().starts_with('=') {
                element.attributes.push(Attribute { name, value: String::new(), range: self.range(self.at, self.at) });
                continue;
            }
            self.at += 1;
            self.skip_space();
            let Some(quote) = self.rest().chars().next().filter(|c| matches!(c, '"' | '\'')) else { continue };
            let from = self.at + 1;
            let to = self.text[from..].find(quote).map_or(self.text.len(), |end| from + end);
            element.attributes.push(Attribute {
                name,
                value: decode(&self.text[from..to]),
                range: self.range(from, to),
            });
            self.at = (to + 1).min(self.text.len());
        }
        self.open.push(Open { element, text: String::new(), text_at: None });
    }
}

/// The top elements of the document `text`: one, for a well-formed one.
pub fn parse(text: &str) -> Vec<Element> {
    let mut reader = Reader { text, lines: LineIndex::new(text), at: 0, open: Vec::new(), roots: Vec::new() };
    reader.read();
    reader.roots
}

/// The document's element: the first at its top.
pub fn root(text: &str) -> Option<Element> {
    parse(text).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DI: &str = "<?xml version=\"1.0\"?>\n<!-- A comment with <tags> in it. -->\n<config xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n    <preference for=\"Shop\\Api\\CartInterface\" type=\"Shop\\Model\\Cart\" />\n    <type name=\"Shop\\Model\\Cart\">\n        <arguments>\n            <argument name=\"limit\" xsi:type=\"number\">5</argument>\n            <argument name=\"label\" xsi:type=\"string\"><![CDATA[a < b]]></argument>\n            <argument name=\"note\" xsi:type='string'>Tom &amp; Jerry &#233;</argument>\n        </arguments>\n    </type>\n</config>\n";

    #[test]
    fn a_document_has_its_elements_attributes_and_text() {
        let config = root(DI).unwrap();
        assert_eq!(config.name, "config");
        assert_eq!(config.range.start.line, 2);
        let preference = config.child("preference").unwrap();
        assert_eq!(preference.attr("for"), Some("Shop\\Api\\CartInterface"));
        let to = preference.attribute("type").unwrap();
        assert_eq!(to.value, "Shop\\Model\\Cart");
        assert_eq!((to.range.start.line, to.range.start.col, to.range.end.col), (3, 51, 66));
        let arguments: Vec<(&str, &str, &str)> = config
            .descendants()
            .into_iter()
            .filter(|element| element.name == "argument")
            .map(|argument| {
                (argument.attr("name").unwrap(), argument.attr("xsi:type").unwrap(), argument.text.as_str())
            })
            .collect();
        assert_eq!(
            arguments,
            [("limit", "number", "5"), ("label", "string", "a < b"), ("note", "string", "Tom & Jerry é")]
        );
        let limit = config.descendants().into_iter().find(|element| element.attr("name") == Some("limit")).unwrap();
        assert_eq!((limit.text_range.start.line, limit.text_range.start.col), (6, 53));
        assert_eq!(config.named("type").count(), 1);
    }

    #[test]
    fn a_document_being_edited_reads_as_far_as_it_goes() {
        // A tag being typed, an element never closed, a closing tag of nothing.
        let text = "<config>\n  <type name=\"A\">\n    <plugin name=\"p\" type=\n  </nothing>\n  <event name=\"saved\"";
        let config = root(text).unwrap();
        let names: Vec<&str> = config.descendants().into_iter().map(|element| element.name.as_str()).collect();
        assert_eq!(names, ["config", "type", "plugin", "event"]);
        let event = config.descendants().into_iter().find(|element| element.name == "event").unwrap();
        assert_eq!(event.attr("name"), Some("saved"));
        assert_eq!(parse(""), Vec::new());
        assert_eq!(parse("no markup at all"), Vec::new());
        // Several top elements are each read.
        assert_eq!(parse("<a/><b></b>").len(), 2);
    }
}
