//! A reader of gettext catalogues (`.po`): each message's text (`msgid`), its
//! translation (`msgstr`), its context (`msgctxt`) and where its text is
//! written. Yii's gettext message source and CakePHP's translations share it.

/// A message of a `.po` file: its context (`msgctxt`), its text, its translation, the
/// line its text is on and the columns of the text.
pub type PoMessage = (Option<String>, String, Option<String>, u32, (u32, u32));

/// The messages of a `.po` file.
pub fn messages(text: &str) -> Vec<PoMessage> {
    fn quoted(line: &str) -> Option<(usize, usize)> {
        let start = line.find('"')? + 1;
        let end = line.rfind('"').filter(|end| *end >= start)?;
        Some((start, end))
    }
    fn unescape(text: &str) -> String {
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        }
        out
    }
    #[derive(PartialEq)]
    enum Part {
        None,
        Context,
        Id,
        Translation,
    }
    let mut out = Vec::new();
    let (mut context, mut id, mut translation) = (None::<String>, None::<String>, None::<String>);
    let mut place = (0u32, (0u32, 0u32));
    let mut part = Part::None;
    let mut flush = |context: &mut Option<String>, id: &mut Option<String>, translation: &mut Option<String>, place| {
        let (line, columns): (u32, (u32, u32)) = place;
        let context = context.take();
        if let Some(id) = id.take().filter(|id| !id.is_empty()) {
            out.push((context, id, translation.take().filter(|text| !text.is_empty()), line, columns));
        }
        *translation = None;
    };
    for (number, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let column = |byte: usize| line[..byte].encode_utf16().count() as u32;
        let content = quoted(line).map(|(start, end)| (unescape(&line[start..end]), (column(start), column(end))));
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if trimmed.is_empty() {
                flush(&mut context, &mut id, &mut translation, place);
                part = Part::None;
            }
            continue;
        }
        let Some((content, columns)) = content else { continue };
        if trimmed.starts_with("msgctxt") {
            flush(&mut context, &mut id, &mut translation, place);
            context = Some(content);
            part = Part::Context;
        } else if trimmed.starts_with("msgid_plural") {
            part = Part::None;
        } else if trimmed.starts_with("msgid") {
            if part != Part::Context {
                flush(&mut context, &mut id, &mut translation, place);
            }
            id = Some(content);
            place = (number as u32, columns);
            part = Part::Id;
        } else if trimmed.starts_with("msgstr") {
            if translation.is_none() {
                translation = Some(content);
                part = Part::Translation;
            } else {
                part = Part::None;
            }
        } else if trimmed.starts_with('"') {
            match part {
                Part::Context => context.get_or_insert_with(String::new).push_str(&content),
                Part::Id => {
                    // `msgid ""` and the text on the lines after it.
                    if id.as_deref() == Some("") {
                        place = (number as u32, columns);
                    }
                    id.get_or_insert_with(String::new).push_str(&content);
                }
                Part::Translation => translation.get_or_insert_with(String::new).push_str(&content),
                Part::None => {}
            }
        }
    }
    flush(&mut context, &mut id, &mut translation, place);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_has_its_messages_with_their_contexts() {
        let text = "msgid \"\"\nmsgstr \"\"\n\"Language: fr\\n\"\n\n#: a.php\nmsgctxt \"shop\"\nmsgid \"Cart\"\nmsgstr \"Panier\"\n\nmsgid \"Hello\"\nmsgstr \"\"\n";
        let found = messages(text);
        assert_eq!(
            found,
            [
                (Some("shop".to_owned()), "Cart".to_owned(), Some("Panier".to_owned()), 6, (7, 11)),
                (None, "Hello".to_owned(), None, 9, (7, 12)),
            ]
        );
    }
}
