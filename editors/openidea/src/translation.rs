//! "Create translation key…": the quick-fix on a translation key some locale lacks
//! asks which locales get the key and their texts.

use crate::edit::{self, Applied};
use crate::server;
use openidea_plugin::{Error, Invocation, PickItem, host};
use serde::Deserialize;
use serde_json::json;

/// The command the action carries: the key and the project's locales, as a JSON string.
pub const ACTION_COMMAND: &str = "laravel.createTranslation";

#[derive(Deserialize)]
struct Locale {
    locale: String,
    /// The locale's text when it has the key, else the suggested one.
    text: String,
    #[serde(default)]
    exists: bool,
}

#[derive(Deserialize)]
struct Request {
    key: String,
    locales: Vec<Locale>,
}

pub fn create(ctx: &Invocation, arg: Option<&str>) -> Result<(), Error> {
    let invalid = |e: &dyn std::fmt::Display| Error::Invalid(format!("{ACTION_COMMAND}: {e}"));
    let arguments = server::command_arguments(arg, ACTION_COMMAND)?;
    let request: Request = match arguments.first() {
        Some(serde_json::Value::String(text)) => serde_json::from_str(text).map_err(|e| invalid(&e))?,
        Some(value) => serde_json::from_value(value.clone()).map_err(|e| invalid(&e))?,
        None => return Err(invalid(&"no arguments")),
    };
    let missing: Vec<&Locale> = request.locales.iter().filter(|l| !l.exists).collect();
    if missing.is_empty() {
        return Err(Error::Invalid(format!("Every locale has '{}' already", request.key)));
    }
    let chosen: Vec<&Locale> = if let [only] = missing.as_slice() {
        vec![only]
    } else {
        let items: Vec<PickItem> = missing
            .iter()
            .map(|l| PickItem { key: l.locale.clone(), label: l.locale.clone(), description: Some(l.text.clone()) })
            .collect();
        let Some(picked) = host::pick_many(&format!("Locales that get '{}'", request.key), &items)? else {
            return Ok(());
        };
        missing.into_iter().filter(|l| picked.contains(&l.locale)).collect()
    };
    let mut texts = Vec::new();
    for locale in chosen {
        let prompt = format!("Text of '{}' in {}", request.key, locale.locale);
        let Some(text) = host::input(&prompt, Some(&locale.text))? else { return Ok(()) };
        texts.push(json!({ "locale": locale.locale, "text": text }));
    }
    if texts.is_empty() {
        return Ok(());
    }
    let applied: Applied = server::request(ctx, "createTranslation", json!({ "key": request.key, "texts": texts }))?;
    edit::apply(&server::project(ctx)?, &applied)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::mock;

    #[test]
    fn the_key_is_written_to_the_locales_chosen_with_the_texts_typed() {
        let mock = mock();
        mock.add_file("/project/lang/fr/posts.php", "<?php\nreturn [\n];\n");
        mock.set_language_server_response(
            server::SERVER,
            "phoxite/laravel/createTranslation",
            r#"{"edit":{"changes":{"file:///project/lang/fr/posts.php":[
                {"range":{"start":{"line":2,"character":0},"end":{"line":2,"character":0}},"newText":"    'title' => 'Titre',\n"}
            ]}},"created":[]}"#,
        );
        let request = json!({ "key": "posts.title", "locales": [
            { "locale": "en", "text": "Title", "exists": true },
            { "locale": "fr", "text": "Title", "exists": false },
            { "locale": "de", "text": "Title", "exists": false },
        ] });
        mock.answer_next_prompt(Some(&["fr"]));
        mock.answer_next_prompt(Some(&["Titre"]));
        create(&mock.invocation(), Some(&json!([request.to_string()]).to_string())).unwrap();

        let sent = mock.language_server_requests();
        let params: serde_json::Value = serde_json::from_str(sent[0].params.as_deref().unwrap()).unwrap();
        assert_eq!(params, json!({ "key": "posts.title", "texts": [{ "locale": "fr", "text": "Titre" }] }));
        let project = mock.invocation().project().unwrap();
        assert_eq!(
            project.read_file("/project/lang/fr/posts.php").unwrap(),
            "<?php\nreturn [\n    'title' => 'Titre',\n];\n"
        );
        assert!(mock.opened_locations().is_empty(), "the editor stays where the key is used");
    }

    #[test]
    fn a_key_every_locale_has_is_refused_and_a_dismissed_prompt_writes_nothing() {
        let mock = mock();
        let all = json!({ "key": "a.b", "locales": [{ "locale": "en", "text": "B", "exists": true }] });
        let arg = json!([all.to_string()]).to_string();
        assert!(matches!(create(&mock.invocation(), Some(&arg)), Err(Error::Invalid(m)) if m.contains("already")));

        // One locale lacks it: no locale question, and the text prompt is dismissed.
        let one = json!({ "key": "a.b", "locales": [{ "locale": "fr", "text": "B", "exists": false }] });
        create(&mock.invocation(), Some(&json!([one.to_string()]).to_string())).unwrap();
        assert!(mock.language_server_requests().is_empty());
        assert!(matches!(create(&mock.invocation(), Some("[]")), Err(Error::Invalid(_))));
    }
}
