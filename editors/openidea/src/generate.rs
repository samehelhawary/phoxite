//! The code generators: Generate… (a class, a migration, a view, the project's
//! `ide.json` code generations) and New Eloquent Model….

use crate::edit::{self, Applied};
use crate::model::parse_columns;
use crate::server;
use openidea_plugin::{Error, Invocation, PickItem, host};
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize)]
struct FieldOption {
    value: String,
    label: String,
}

/// An input of an `ide.json` code generation.
#[derive(Deserialize)]
struct Field {
    id: String,
    title: String,
    kind: String,
    optional: bool,
    default: String,
    options: Vec<FieldOption>,
}

#[derive(Deserialize)]
struct Generator {
    id: String,
    title: String,
    description: String,
    /// What it asks for besides the name: `model`, `related`, `table`…
    options: Vec<String>,
    fields: Vec<Field>,
}

#[derive(Deserialize)]
struct Context {
    models: Vec<String>,
    generators: Vec<Generator>,
}

fn context(ctx: &Invocation) -> Result<Context, Error> {
    let context: Option<Context> = server::request(ctx, "generationContext", json!({}))?;
    context.ok_or_else(|| Error::Invalid("This is not a Laravel project".into()))
}

/// What the name of generator `id` is, as the prompt says it; none for a generator
/// that takes no name.
fn name_prompt(id: &str) -> Option<&'static str> {
    match id {
        "pivot-migration" => None,
        "view" => Some("View name, e.g. posts.index"),
        "anonymous-component" => Some("Component name, e.g. forms.input"),
        "livewire" => Some("Component name, e.g. posts.create-post"),
        "migration" => Some("Migration name, e.g. add_votes_to_users_table"),
        _ => Some("Class name, e.g. Post or Admin/Post"),
    }
}

/// One of the project's models, or none (the first item).
fn pick_model(title: &str, models: &[String]) -> Result<Option<String>, Error> {
    const NONE: &str = "\u{0}none";
    let mut items = vec![PickItem { key: NONE.into(), label: "(none)".into(), description: None }];
    items.extend(models.iter().map(|class| PickItem {
        key: class.clone(),
        label: server::short_name(class).to_owned(),
        description: Some(class.clone()),
    }));
    Ok(host::pick(title, &items)?.map(|key| if key == NONE { String::new() } else { key }))
}

/// Asks for the inputs of an `ide.json` code generation. None when the user gave up.
fn inputs(fields: &[Field]) -> Result<Option<Map<String, Value>>, Error> {
    let mut values = Map::new();
    for field in fields {
        let value = match field.kind.as_str() {
            "checkbox" => {
                let items = [
                    PickItem { key: "true".into(), label: "Yes".into(), description: None },
                    PickItem { key: "false".into(), label: "No".into(), description: None },
                ];
                host::pick(&field.title, &items)?
            }
            "select" => {
                let items: Vec<PickItem> = field
                    .options
                    .iter()
                    .map(|o| PickItem { key: o.value.clone(), label: o.label.clone(), description: None })
                    .collect();
                host::pick(&field.title, &items)?
            }
            _ => {
                let title = if field.optional { format!("{} (optional)", field.title) } else { field.title.clone() };
                host::input(&title, Some(&field.default).filter(|d| !d.is_empty()).map(String::as_str))?
            }
        };
        let Some(value) = value else { return Ok(None) };
        if value.trim().is_empty() && !field.optional && field.kind != "checkbox" {
            return Err(Error::Invalid(format!("{} is needed", field.title)));
        }
        values.insert(field.id.clone(), Value::String(value));
    }
    Ok(Some(values))
}

/// Generate…: pick a generator, answer what it asks, and open what it wrote.
pub fn generate(ctx: &Invocation) -> Result<(), Error> {
    let context = context(ctx)?;
    let items: Vec<PickItem> = context
        .generators
        .iter()
        .map(|g| PickItem {
            key: g.id.clone(),
            label: g.title.clone(),
            description: Some(g.description.clone()).filter(|d| !d.is_empty()),
        })
        .collect();
    let Some(id) = host::pick("Generate", &items)? else { return Ok(()) };
    let Some(generator) = context.generators.iter().find(|g| g.id == id) else { return Ok(()) };

    let name = match name_prompt(&generator.id) {
        Some(prompt) => {
            let Some(name) = host::input(prompt, None)? else { return Ok(()) };
            if name.trim().is_empty() {
                return Err(Error::Invalid("Enter a name".into()));
            }
            name.trim().to_owned()
        }
        None => String::new(),
    };
    let wants = |option: &str| generator.options.iter().any(|o| o == option);
    let mut model = String::new();
    if wants("model") {
        let Some(chosen) = pick_model(&format!("Model of the {}", generator.title), &context.models)? else {
            return Ok(());
        };
        model = chosen;
    }
    let mut related = String::new();
    if wants("related") {
        let Some(chosen) = pick_model("Related model", &context.models)? else { return Ok(()) };
        related = chosen;
    }
    let Some(inputs) = inputs(&generator.fields)? else { return Ok(()) };

    // The module is left to the project's settings; the options this dialog doesn't
    // ask about keep the defaults the generators have without it.
    let spec = json!({
        "generator": generator.id, "name": name, "module": null,
        "model": model, "related": related, "table": "", "create": false, "event": "",
        "queued": true, "api": false, "backing": "", "status": 0, "inputs": inputs,
    });
    let applied: Applied = server::request(ctx, "generateClass", json!({ "spec": spec }))?;
    edit::apply_and_open(&server::project(ctx)?, &applied)?;
    Ok(())
}

/// What New Eloquent Model can create along with the model.
const EXTRAS: [(&str, &str); 9] = [
    ("migration", "Migration"),
    ("factory", "Factory"),
    ("seeder", "Seeder"),
    ("controller", "Resource controller"),
    ("formRequests", "Form Requests"),
    ("resource", "JSON resource"),
    ("policy", "Policy"),
    ("timestamps", "Timestamps (created_at, updated_at)"),
    ("softDeletes", "Soft deletes"),
];

/// New Eloquent Model…: a name, its fields, and what to create with it.
pub fn new_model(ctx: &Invocation) -> Result<(), Error> {
    let Some(name) = host::input("Model name, e.g. Post or Blog/Post", None)? else { return Ok(()) };
    if name.trim().is_empty() {
        return Err(Error::Invalid("Enter a name".into()));
    }
    let prompt = "Fields, e.g. title:string, body:text?, votes:integer (empty for none)";
    let Some(typed) = host::input(prompt, None)? else { return Ok(()) };
    let fields: Vec<Value> = if typed.trim().is_empty() {
        Vec::new()
    } else {
        let columns = serde_json::to_value(parse_columns(&typed)?).map_err(|e| Error::Failed(e.to_string()))?;
        let rows = columns.as_array().cloned().unwrap_or_default();
        rows.into_iter()
            .map(|mut column| {
                for (key, value) in [("unique", json!(false)), ("index", json!(false)), ("default", json!(""))] {
                    column[key] = value;
                }
                column
            })
            .collect()
    };
    let items: Vec<PickItem> = EXTRAS
        .iter()
        .map(|(key, label)| PickItem { key: (*key).into(), label: (*label).into(), description: None })
        .collect();
    let Some(chosen) = host::pick_many("Create with the model", &items)? else { return Ok(()) };
    let has = |key: &str| chosen.iter().any(|c| c == key);
    let spec = json!({
        "name": name.trim(), "module": "", "fields": fields, "relations": [], "indexes": [],
        "timestamps": has("timestamps"), "softDeletes": has("softDeletes"),
        "migration": has("migration"), "factory": has("factory"), "seeder": has("seeder"),
        "controller": if has("controller") { "resource" } else { "none" },
        "formRequests": has("formRequests"), "resource": has("resource"), "policy": has("policy"),
    });
    let applied: Applied = server::request(ctx, "newModel", json!({ "spec": spec }))?;
    edit::apply_and_open(&server::project(ctx)?, &applied)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::mock;

    const CONTEXT: &str = r#"{
        "namespace":"App\\","modules":[],"defaultModule":null,
        "models":["App\\Models\\Post","App\\Models\\User"],
        "generators":[
            {"id":"controller","title":"Controller","description":"An HTTP controller","templates":["Laravel Controller"],
             "options":["model"],"fields":[]},
            {"id":"ide-json:action","title":"Action","description":"","templates":[],"options":[],
             "fields":[
                {"id":"queue","title":"Queue","kind":"select","variable":"QUEUE","optional":false,"default":"",
                 "options":[{"value":"high","label":"High"},{"value":"low","label":"Low"}]},
                {"id":"note","title":"Note","kind":"text","variable":"NOTE","optional":true,"default":"","options":[]}
             ]}
        ]
    }"#;

    fn created(path: &str, text: &str) -> String {
        json!({
            "edit": { "documentChanges": [
                { "kind": "create", "uri": format!("file://{path}") },
                { "textDocument": { "uri": format!("file://{path}"), "version": null },
                  "edits": [{ "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }, "newText": text }] },
            ] },
            "created": [format!("file://{path}")],
        })
        .to_string()
    }

    fn last_params(mock: &openidea_plugin::mock::MockHost) -> Value {
        let sent = mock.language_server_requests();
        serde_json::from_str(sent.last().unwrap().params.as_deref().unwrap()).unwrap()
    }

    #[test]
    fn a_generator_asks_its_name_and_model_and_writes_the_class() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/generationContext", CONTEXT);
        let path = "/project/app/Http/Controllers/PostController.php";
        mock.set_language_server_response(
            server::SERVER,
            "phoxite/laravel/generateClass",
            &created(path, "<?php // controller\n"),
        );
        mock.answer_next_prompt(Some(&["controller"]));
        mock.answer_next_prompt(Some(&["PostController"]));
        mock.answer_next_prompt(Some(&["App\\Models\\Post"]));
        generate(&mock.invocation()).unwrap();

        let spec = &last_params(&mock)["spec"];
        assert_eq!(
            (&spec["generator"], &spec["name"], &spec["model"], &spec["module"], &spec["queued"]),
            (&json!("controller"), &json!("PostController"), &json!("App\\Models\\Post"), &Value::Null, &json!(true))
        );
        let project = mock.invocation().project().unwrap();
        assert_eq!(project.read_file(path).unwrap(), "<?php // controller\n");
        assert_eq!(mock.opened_locations(), [(path.to_owned(), 0, 0)]);
    }

    #[test]
    fn an_ide_json_generation_asks_its_fields() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/generationContext", CONTEXT);
        mock.set_language_server_response(
            server::SERVER,
            "phoxite/laravel/generateClass",
            &created("/project/app/Actions/Publish.php", "<?php\n"),
        );
        mock.answer_next_prompt(Some(&["ide-json:action"]));
        mock.answer_next_prompt(Some(&["Publish"]));
        mock.answer_next_prompt(Some(&["low"]));
        mock.answer_next_prompt(Some(&[""]));
        generate(&mock.invocation()).unwrap();
        assert_eq!(last_params(&mock)["spec"]["inputs"], json!({ "queue": "low", "note": "" }));

        // Giving up at any question writes nothing.
        let before = mock.language_server_requests().len();
        mock.answer_next_prompt(Some(&["ide-json:action"]));
        mock.answer_next_prompt(Some(&["Other"]));
        generate(&mock.invocation()).unwrap();
        assert_eq!(mock.language_server_requests().len(), before + 1, "only the context was asked for");
    }

    #[test]
    fn a_new_model_takes_its_fields_and_what_to_create_with_it() {
        let mock = mock();
        mock.set_language_server_response(
            server::SERVER,
            "phoxite/laravel/newModel",
            &created("/project/app/Models/Post.php", "<?php // model\n"),
        );
        mock.answer_next_prompt(Some(&["Post"]));
        mock.answer_next_prompt(Some(&["title:string, body:text?"]));
        mock.answer_next_prompt(Some(&["migration", "controller", "timestamps"]));
        new_model(&mock.invocation()).unwrap();

        let spec = &last_params(&mock)["spec"];
        assert_eq!(spec["name"], "Post");
        assert_eq!(
            spec["fields"],
            json!([
                { "name": "title", "ty": "string", "nullable": false, "unique": false, "index": false, "default": "" },
                { "name": "body", "ty": "text", "nullable": true, "unique": false, "index": false, "default": "" },
            ])
        );
        assert_eq!(
            (&spec["migration"], &spec["factory"], &spec["controller"], &spec["timestamps"], &spec["softDeletes"]),
            (&json!(true), &json!(false), &json!("resource"), &json!(true), &json!(false))
        );
        assert_eq!(mock.opened_locations(), [("/project/app/Models/Post.php".to_owned(), 0, 0)]);
    }
}
