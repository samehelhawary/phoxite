//! The code generators: Generate… (a class, a migration, a view, the project's
//! `ide.json` code generations) and New Eloquent Model….
//!
//! Each asks what it needs one step at a time, in the order the VS Code extension
//! does: the name, where it goes, then what the server lists for the generator.

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
    /// What it asks for besides the name: `model`, `related`, `table`, `create`,
    /// `event`, `queued`, `api`, `backing`, `status`.
    options: Vec<String>,
    fields: Vec<Field>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Context {
    /// The application's namespace, with a trailing `\`.
    namespace: String,
    modules: Vec<String>,
    default_module: Option<String>,
    models: Vec<String>,
    generators: Vec<Generator>,
}

/// The generator that is New Eloquent Model.
const MODEL_GENERATOR: &str = "model";
const RELATION_KINDS: [&str; 9] = [
    "belongsTo",
    "hasOne",
    "hasMany",
    "belongsToMany",
    "hasManyThrough",
    "morphOne",
    "morphMany",
    "morphToMany",
    "morphTo",
];

/// What a foreign key (`foreignId`, `foreignUuid`) can do when the row it points at
/// is deleted, as the server's generator names it.
const ON_DELETE: [&str; 4] = ["cascade", "restrict", "setNull", "noAction"];

fn context(ctx: &Invocation) -> Result<Context, Error> {
    let context: Option<Context> = server::request(ctx, "generationContext", json!({}))?;
    context.ok_or_else(|| Error::Invalid("This is not a Laravel project".into()))
}

fn item(key: &str, label: &str, description: Option<&str>) -> PickItem {
    PickItem { key: key.into(), label: label.into(), description: description.map(str::to_owned) }
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

/// A key no model or module has: the entry for "none" / the application.
const NONE: &str = "\u{0}none";

fn or_none(key: String) -> String {
    if key == NONE { String::new() } else { key }
}

/// One of `models`; with `none`, also an entry for no model (the first).
fn pick_model(title: &str, models: &[&String], none: Option<&str>) -> Result<Option<String>, Error> {
    let mut items: Vec<PickItem> = none.map(|label| item(NONE, label, None)).into_iter().collect();
    items.extend(models.iter().map(|class| item(class, server::short_name(class), Some(class))));
    Ok(host::pick(title, &items)?.map(or_none))
}

/// Where the class goes when the project has modules: the application ("") or one of
/// them, the module of the project's settings first. None when the user gave up.
fn pick_module(context: &Context) -> Result<Option<String>, Error> {
    if context.modules.is_empty() {
        return Ok(Some(context.default_module.clone().unwrap_or_default()));
    }
    let mut items = vec![item(NONE, "Application", Some(&context.namespace))];
    items.extend(context.modules.iter().map(|module| item(module, module, Some("module"))));
    let preferred = context.default_module.clone().unwrap_or_default();
    items.sort_by_key(|i| or_none(i.key.clone()) != preferred);
    Ok(host::pick("Where it goes", &items)?.map(or_none))
}

/// Asks for the text and choice inputs of an `ide.json` code generation (its
/// checkboxes are ticked with the other options). None when the user gave up.
fn inputs(fields: &[Field]) -> Result<Option<Map<String, Value>>, Error> {
    let mut values = Map::new();
    for field in fields.iter().filter(|f| f.kind != "checkbox") {
        let value = if field.kind == "select" {
            let mut items: Vec<PickItem> = Vec::new();
            if field.optional {
                items.push(item(NONE, "None", None));
            }
            items.extend(field.options.iter().map(|o| {
                item(&o.value, &o.label, (o.value == field.default && !o.value.is_empty()).then_some("default"))
            }));
            host::pick(&field.title, &items)?.map(or_none)
        } else {
            let title = if field.optional { format!("{} (optional)", field.title) } else { field.title.clone() };
            host::input(&title, Some(field.default.as_str()).filter(|d| !d.is_empty()))?
        };
        let Some(value) = value else { return Ok(None) };
        if value.trim().is_empty() && !field.optional {
            return Err(Error::Invalid(format!("Enter {}", field.title)));
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
        .map(|g| item(&g.id, &g.title, Some(g.description.as_str()).filter(|d| !d.is_empty())))
        .collect();
    let Some(id) = host::pick("Generate", &items)? else { return Ok(()) };
    if id == MODEL_GENERATOR {
        return new_model_in(ctx, &context);
    }
    let Some(generator) = context.generators.iter().find(|g| g.id == id) else { return Ok(()) };
    let wants = |option: &str| generator.options.iter().any(|o| o == option);
    let pivot = name_prompt(&generator.id).is_none();

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
    let Some(module) = pick_module(&context)? else { return Ok(()) };

    let (mut model, mut related) = (String::new(), String::new());
    if wants("model") {
        if pivot && context.models.len() < 2 {
            return Err(Error::Invalid("A pivot table needs two Eloquent models, and this project has fewer".into()));
        }
        let all: Vec<&String> = context.models.iter().collect();
        let (title, none) = if pivot { ("The first model", None) } else { ("The model", Some("No model")) };
        let Some(chosen) = pick_model(title, &all, none)? else { return Ok(()) };
        model = chosen;
        if wants("related") {
            let others: Vec<&String> = context.models.iter().filter(|m| **m != model).collect();
            let Some(chosen) = pick_model("The related model", &others, None)? else { return Ok(()) };
            related = chosen;
        }
    }
    let mut table = String::new();
    if wants("table") {
        let Some(typed) = host::input("Table (guessed from the name when empty)", None)? else { return Ok(()) };
        table = typed.trim().to_owned();
    }
    let mut event = String::new();
    if wants("event") {
        let Some(typed) = host::input("Event class (optional), e.g. OrderShipped", None)? else { return Ok(()) };
        event = typed.trim().to_owned();
    }
    let mut backing = String::new();
    if wants("backing") {
        let kinds = [
            item(NONE, "Pure enum", None),
            item("string", "Backed by string", None),
            item("int", "Backed by int", None),
        ];
        let Some(chosen) = host::pick("Backing type", &kinds)? else { return Ok(()) };
        backing = or_none(chosen);
    }
    let mut status = 0;
    if wants("status") {
        let Some(typed) = host::input("HTTP status, e.g. 403 (500 when empty)", None)? else { return Ok(()) };
        let typed = typed.trim();
        let valid = typed.len() == 3
            && typed.starts_with(['1', '2', '3', '4', '5'])
            && typed.chars().all(|c| c.is_ascii_digit());
        if !typed.is_empty() && !valid {
            return Err(Error::Invalid("The status is a three-digit HTTP status".into()));
        }
        status = typed.parse().unwrap_or(0);
    }
    let Some(mut inputs) = inputs(&generator.fields)? else { return Ok(()) };

    // Everything ticked or not, in one list. A job or a listener is queued unless told
    // otherwise, and a list can't come with a tick, so the entry is the exception.
    let mut flags: Vec<PickItem> = Vec::new();
    if wants("create") {
        flags.push(item("create", "Create the table", None));
    }
    if wants("queued") {
        flags.push(item("sync", "Not queued", Some("runs at once")));
    }
    if wants("api") {
        flags.push(item("api", "API", Some("no create and edit actions")));
    }
    let checkboxes: Vec<&Field> = generator.fields.iter().filter(|f| f.kind == "checkbox").collect();
    flags.extend(checkboxes.iter().map(|f| item(&format!("field:{}", f.id), &f.title, None)));
    let ticked = if flags.is_empty() {
        Vec::new()
    } else {
        let Some(ticked) = host::pick_many("Options", &flags)? else { return Ok(()) };
        ticked
    };
    let has = |key: &str| ticked.iter().any(|t| t == key);
    for field in checkboxes {
        let on = has(&format!("field:{}", field.id));
        inputs.insert(field.id.clone(), Value::String(if on { "true".into() } else { String::new() }));
    }

    let spec = json!({
        "generator": generator.id, "name": name, "module": module,
        "model": model, "related": related, "table": table, "create": has("create"), "event": event,
        "queued": wants("queued") && !has("sync"), "api": has("api"), "backing": backing, "status": status,
        "inputs": inputs,
    });
    let applied: Applied = server::request(ctx, "generateClass", json!({ "spec": spec }))?;
    edit::apply_and_open(&server::project(ctx)?, &applied)?;
    Ok(())
}

/// The fields of `title:string, slug:string unique, votes:integer? index`: a column as
/// Add Columns takes it, then `unique` or `index`. Two more types than Add Columns has:
/// `status:enum(draft|published)` lists its values, and `subject:morphs` is the two
/// columns of a polymorphic relation. A foreign key can say what deleting does:
/// `user_id:foreignId cascade` (or `restrict`, `setNull`, `noAction`).
fn parse_fields(typed: &str) -> Result<Vec<Value>, Error> {
    let mut fields = Vec::new();
    for part in typed.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        // The values of an enum, taken out before the words are told apart.
        let (part, values) = match (part.find('('), part.rfind(')')) {
            (Some(open), Some(close)) if open < close => {
                let values: Vec<&str> =
                    part[open + 1..close].split('|').map(str::trim).filter(|v| !v.is_empty()).collect();
                (format!("{}{}", &part[..open], &part[close + 1..]), values)
            }
            _ => (part.to_owned(), Vec::new()),
        };
        let mut words = part.split_whitespace();
        let mut column = words.next().unwrap_or_default().to_owned();
        let (mut unique, mut index, mut on_delete) = (false, false, "");
        for word in words {
            match word {
                "?" => column.push('?'),
                "unique" => unique = true,
                "index" => index = true,
                word if ON_DELETE.contains(&word) => on_delete = word,
                other => {
                    return Err(Error::Invalid(format!(
                        "'{other}' after a field is not `unique`, `index` or what a foreign key does on delete ({})",
                        ON_DELETE.join(", ")
                    )));
                }
            }
        }
        let ty = column.split_once(':').map(|(_, ty)| ty.trim_end_matches('?')).unwrap_or_default();
        let mut field = if matches!(ty, "enum" | "morphs") {
            let (name, _) = column.split_once(':').unwrap_or_default();
            json!({ "name": name.trim(), "ty": ty, "nullable": column.ends_with('?') })
        } else {
            let parsed = parse_columns(&column)?;
            serde_json::to_value(&parsed[0]).map_err(|e| Error::Failed(e.to_string()))?
        };
        if ty == "enum" && values.is_empty() {
            return Err(Error::Invalid(format!("List the values of '{column}': status:enum(draft|published)")));
        }
        if ty != "enum" && !values.is_empty() {
            return Err(Error::Invalid(format!("Only an enum lists values, and '{column}' is not one")));
        }
        if !on_delete.is_empty() && !ty.starts_with("foreign") {
            return Err(Error::Invalid(format!(
                "'{column}' is not a foreign key: `{on_delete}` is for foreignId and foreignUuid"
            )));
        }
        field["unique"] = json!(unique);
        field["index"] = json!(index);
        field["default"] = json!("");
        field["values"] = json!(values);
        field["onDelete"] = json!(on_delete);
        fields.push(field);
    }
    Ok(fields)
}

/// The relations of `belongsTo:User, hasMany:Comment as comments`: a kind, the related
/// model, and the method's name when it isn't the one the model gives. A
/// `hasManyThrough` names the model between: `hasManyThrough:Post through User`.
fn parse_relations(typed: &str) -> Result<Vec<Value>, Error> {
    let mut relations = Vec::new();
    for part in typed.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (relation, name) = match part.split_once(" as ") {
            Some((relation, name)) => (relation.trim(), name.trim()),
            None => (part, ""),
        };
        let (relation, through) = match relation.split_once(" through ") {
            Some((relation, through)) => (relation.trim(), through.trim()),
            None => (relation, ""),
        };
        let Some((kind, model)) = relation.split_once(':').map(|(kind, model)| (kind.trim(), model.trim())) else {
            return Err(Error::Invalid(format!("'{part}' is not a relation: write it as hasMany:Comment")));
        };
        if !RELATION_KINDS.contains(&kind) {
            return Err(Error::Invalid(format!(
                "'{kind}' is not a relation: use one of {}",
                RELATION_KINDS.join(", ")
            )));
        }
        let identifier = |text: &str| {
            let mut chars = text.chars();
            chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '\\')
        };
        // `morphTo` names no model.
        if (!model.is_empty() || kind != "morphTo") && !identifier(model) {
            return Err(Error::Invalid(format!("'{model}' is not a model's name")));
        }
        if !name.is_empty() && !identifier(name) {
            return Err(Error::Invalid(format!("'{name}' is not a method's name")));
        }
        if (kind == "hasManyThrough") != !through.is_empty() || (!through.is_empty() && !identifier(through)) {
            return Err(Error::Invalid(format!(
                "A hasManyThrough relation, and only one, names the model between: hasManyThrough:{} through User",
                if model.is_empty() { "Post" } else { model }
            )));
        }
        relations.push(json!({ "kind": kind, "model": model, "name": name, "through": through }));
    }
    Ok(relations)
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

/// New Eloquent Model…: a name, its fields and relations, and what to create with it.
pub fn new_model(ctx: &Invocation) -> Result<(), Error> {
    new_model_in(ctx, &context(ctx)?)
}

fn new_model_in(ctx: &Invocation, context: &Context) -> Result<(), Error> {
    let Some(name) = host::input("Model name, e.g. Post or Blog/Post", None)? else { return Ok(()) };
    if name.trim().is_empty() {
        return Err(Error::Invalid("Enter a name".into()));
    }
    let Some(module) = pick_module(context)? else { return Ok(()) };
    let prompt = "Fields, e.g. title:string, slug:string unique, votes:integer? index, status:enum(draft|published), user_id:foreignId cascade (empty for none)";
    let Some(typed) = host::input(prompt, None)? else { return Ok(()) };
    let fields = parse_fields(&typed)?;
    let prompt = "Relations, e.g. belongsTo:User, hasMany:Comment as comments, hasManyThrough:Post through User (empty for none)";
    let Some(typed) = host::input(prompt, None)? else { return Ok(()) };
    let relations = parse_relations(&typed)?;
    let items: Vec<PickItem> = EXTRAS.iter().map(|(key, label)| item(key, label, None)).collect();
    let Some(chosen) = host::pick_many("Create with the model", &items)? else { return Ok(()) };
    let has = |key: &str| chosen.iter().any(|c| c == key);
    let spec = json!({
        "name": name.trim(), "module": module, "fields": fields, "relations": relations, "indexes": [],
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
        "namespace":"App\\","modules":["Billing"],"defaultModule":"Billing",
        "models":["App\\Models\\Post","App\\Models\\User"],
        "generators":[
            {"id":"model","title":"Model","description":"An Eloquent model","templates":[],"options":[],"fields":[]},
            {"id":"resource-controller","title":"Resource Controller","description":"A controller with the resource actions",
             "templates":["Laravel Controller"],"options":["model","api"],"fields":[]},
            {"id":"migration","title":"Migration","description":"","templates":[],"options":["table","create"],"fields":[]},
            {"id":"listener","title":"Listener","description":"","templates":[],"options":["event","queued"],"fields":[]},
            {"id":"pivot-migration","title":"Pivot Table Migration","description":"","templates":[],
             "options":["model","related"],"fields":[]},
            {"id":"enum","title":"Enum","description":"","templates":[],"options":["backing"],"fields":[]},
            {"id":"http-exception","title":"HTTP Exception","description":"","templates":[],"options":["status"],"fields":[]},
            {"id":"ide-json:action","title":"Action","description":"","templates":[],"options":[],
             "fields":[
                {"id":"queue","title":"Queue","kind":"select","variable":"QUEUE","optional":false,"default":"low",
                 "options":[{"value":"high","label":"High"},{"value":"low","label":"Low"}]},
                {"id":"note","title":"Note","kind":"text","variable":"NOTE","optional":true,"default":"","options":[]},
                {"id":"final","title":"Final class","kind":"checkbox","variable":"FINAL","optional":true,"default":"","options":[]}
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

    /// A host that answers the context and writes whatever is generated to `Made.php`.
    fn generating() -> openidea_plugin::mock::MockHost {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/generationContext", CONTEXT);
        let made = created("/project/app/Made.php", "<?php // made\n");
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/generateClass", &made);
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/newModel", &made);
        mock
    }

    /// Runs Generate with `answers` to its questions, in order, and gives the spec sent.
    fn generated(answers: &[&[&str]]) -> Value {
        let mock = generating();
        for answer in answers {
            mock.answer_next_prompt(Some(answer));
        }
        generate(&mock.invocation()).unwrap();
        last_params(&mock)["spec"].clone()
    }

    #[test]
    fn a_generator_asks_its_name_where_it_goes_and_its_model_and_writes_the_class() {
        let mock = generating();
        for answer in
            [&["resource-controller"][..], &["PostController"], &["Billing"], &["App\\Models\\Post"], &["api"]]
        {
            mock.answer_next_prompt(Some(answer));
        }
        generate(&mock.invocation()).unwrap();
        let spec = &last_params(&mock)["spec"];
        assert_eq!(
            (&spec["generator"], &spec["name"], &spec["module"], &spec["model"], &spec["api"], &spec["queued"]),
            (
                &json!("resource-controller"),
                &json!("PostController"),
                &json!("Billing"),
                &json!("App\\Models\\Post"),
                &json!(true),
                &json!(false)
            )
        );
        let project = mock.invocation().project().unwrap();
        assert_eq!(project.read_file("/project/app/Made.php").unwrap(), "<?php // made\n");
        assert_eq!(mock.opened_locations(), [("/project/app/Made.php".to_owned(), 0, 0)]);
    }

    #[test]
    fn each_option_of_a_generator_is_asked_for() {
        // The application rather than the module, no model, nothing ticked.
        let spec = generated(&[&["resource-controller"], &["Home"], &[NONE], &[NONE], &[]]);
        assert_eq!((&spec["module"], &spec["model"], &spec["api"]), (&json!(""), &json!(""), &json!(false)));

        let spec = generated(&[&["migration"], &["add_votes_to_posts_table"], &["Billing"], &["posts"], &["create"]]);
        assert_eq!((&spec["table"], &spec["create"]), (&json!("posts"), &json!(true)));

        // A listener is queued unless told otherwise.
        let spec = generated(&[&["listener"], &["SendReceipt"], &["Billing"], &["OrderShipped"], &[]]);
        assert_eq!((&spec["event"], &spec["queued"]), (&json!("OrderShipped"), &json!(true)));
        let spec = generated(&[&["listener"], &["SendReceipt"], &["Billing"], &[""], &["sync"]]);
        assert_eq!((&spec["event"], &spec["queued"]), (&json!(""), &json!(false)));

        // A pivot table takes no name and two different models.
        let spec = generated(&[&["pivot-migration"], &["Billing"], &["App\\Models\\Post"], &["App\\Models\\User"]]);
        assert_eq!(
            (&spec["name"], &spec["model"], &spec["related"]),
            (&json!(""), &json!("App\\Models\\Post"), &json!("App\\Models\\User"))
        );

        let spec = generated(&[&["enum"], &["Visibility"], &["Billing"], &["string"]]);
        assert_eq!(spec["backing"], "string");
        let spec = generated(&[&["enum"], &["Visibility"], &["Billing"], &[NONE]]);
        assert_eq!(spec["backing"], "");

        let spec = generated(&[&["http-exception"], &["Forbidden"], &["Billing"], &["403"]]);
        assert_eq!(spec["status"], 403);
        let mock = generating();
        for answer in [&["http-exception"][..], &["Forbidden"], &["Billing"], &["forbidden"]] {
            mock.answer_next_prompt(Some(answer));
        }
        assert!(matches!(generate(&mock.invocation()), Err(Error::Invalid(m)) if m.contains("three-digit")));
    }

    #[test]
    fn an_ide_json_generation_asks_its_fields_and_ticks_its_checkboxes() {
        let spec = generated(&[&["ide-json:action"], &["Publish"], &["Billing"], &["high"], &[""], &["field:final"]]);
        assert_eq!(spec["inputs"], json!({ "queue": "high", "note": "", "final": "true" }));
        let spec = generated(&[&["ide-json:action"], &["Publish"], &["Billing"], &["low"], &["x"], &[]]);
        assert_eq!(spec["inputs"], json!({ "queue": "low", "note": "x", "final": "" }));

        // Giving up at any question writes nothing.
        let mock = generating();
        mock.answer_next_prompt(Some(&["ide-json:action"]));
        mock.answer_next_prompt(Some(&["Other"]));
        generate(&mock.invocation()).unwrap();
        assert_eq!(mock.language_server_requests().len(), 1, "only the context was asked for");
    }

    #[test]
    fn fields_and_relations_are_typed_in_one_line_each() {
        assert_eq!(
            parse_fields("title:string, slug:string unique, votes:integer ? index,").unwrap(),
            [
                json!({ "name": "title", "ty": "string", "nullable": false, "unique": false, "index": false, "default": "", "values": [], "onDelete": "" }),
                json!({ "name": "slug", "ty": "string", "nullable": false, "unique": true, "index": false, "default": "", "values": [], "onDelete": "" }),
                json!({ "name": "votes", "ty": "integer", "nullable": true, "unique": false, "index": true, "default": "", "values": [], "onDelete": "" }),
            ]
        );
        // An enum's values, the columns of a polymorphic relation, a foreign key's on delete.
        assert_eq!(
            parse_fields("status:enum(draft | published)? index, subject:morphs, team_id:foreignUuid cascade").unwrap(),
            [
                json!({ "name": "status", "ty": "enum", "nullable": true, "unique": false, "index": true, "default": "", "values": ["draft", "published"], "onDelete": "" }),
                json!({ "name": "subject", "ty": "morphs", "nullable": false, "unique": false, "index": false, "default": "", "values": [], "onDelete": "" }),
                json!({ "name": "team_id", "ty": "foreignUuid", "nullable": false, "unique": false, "index": false, "default": "", "values": [], "onDelete": "cascade" }),
            ]
        );
        assert!(matches!(parse_fields("status:enum"), Err(Error::Invalid(m)) if m.contains("draft|published")));
        assert!(matches!(parse_fields("title:string(a|b)"), Err(Error::Invalid(m)) if m.contains("Only an enum")));
        assert!(
            matches!(parse_fields("title:string cascade"), Err(Error::Invalid(m)) if m.contains("not a foreign key"))
        );
        assert!(parse_fields("").unwrap().is_empty());
        assert!(matches!(parse_fields("title:string primary"), Err(Error::Invalid(m)) if m.contains("unique")));

        assert_eq!(
            parse_relations("belongsTo:User, hasMany : Comment as comments, morphTo: as owner").unwrap(),
            [
                json!({ "kind": "belongsTo", "model": "User", "name": "", "through": "" }),
                json!({ "kind": "hasMany", "model": "Comment", "name": "comments", "through": "" }),
                json!({ "kind": "morphTo", "model": "", "name": "owner", "through": "" }),
            ]
        );
        assert_eq!(
            parse_relations("hasManyThrough:Post through User as articles, morphToMany:Tag").unwrap(),
            [
                json!({ "kind": "hasManyThrough", "model": "Post", "name": "articles", "through": "User" }),
                json!({ "kind": "morphToMany", "model": "Tag", "name": "", "through": "" }),
            ]
        );
        assert!(matches!(parse_relations("hasManyThrough:Post"), Err(Error::Invalid(m)) if m.contains("through User")));
        assert!(
            matches!(parse_relations("hasMany:Post through User"), Err(Error::Invalid(m)) if m.contains("only one"))
        );
        assert!(matches!(parse_relations("owns:User"), Err(Error::Invalid(m)) if m.contains("belongsTo")));
        assert!(matches!(parse_relations("hasMany"), Err(Error::Invalid(m)) if m.contains("hasMany:Comment")));
        assert!(matches!(parse_relations("hasMany:"), Err(Error::Invalid(m)) if m.contains("model's name")));
    }

    #[test]
    fn a_new_model_takes_its_fields_relations_and_what_to_create_with_it() {
        let mock = generating();
        // Reached through Generate: its Model entry is the same command.
        for answer in [
            &["model"][..],
            &["Post"],
            &[NONE],
            &["title:string, body:text?"],
            &["belongsTo:User"],
            &["migration", "controller", "timestamps"],
        ] {
            mock.answer_next_prompt(Some(answer));
        }
        generate(&mock.invocation()).unwrap();

        let sent = mock.language_server_requests();
        assert_eq!(sent.last().unwrap().method, "phoxite/laravel/newModel");
        let spec = &last_params(&mock)["spec"];
        assert_eq!((&spec["name"], &spec["module"]), (&json!("Post"), &json!("")));
        assert_eq!(
            spec["fields"][1],
            json!({ "name": "body", "ty": "text", "nullable": true, "unique": false, "index": false, "default": "", "values": [], "onDelete": "" })
        );
        assert_eq!(spec["relations"], json!([{ "kind": "belongsTo", "model": "User", "name": "", "through": "" }]));
        assert_eq!(
            (&spec["migration"], &spec["factory"], &spec["controller"], &spec["timestamps"], &spec["softDeletes"]),
            (&json!(true), &json!(false), &json!("resource"), &json!(true), &json!(false))
        );
        assert_eq!(mock.opened_locations(), [("/project/app/Made.php".to_owned(), 0, 0)]);
    }
}
