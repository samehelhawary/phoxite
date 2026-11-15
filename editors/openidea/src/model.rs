//! The Eloquent model view: a model's table, columns, relations, scopes, accessors and
//! related classes, and Add Columns.
//!
//! Which model a project's view shows is kept in the plugin's storage, by project
//! root: the instance can be reclaimed between two renders, and windows must not mix.

use crate::edit::{self, Applied};
use crate::server::{self, Place, Target};
use openidea_plugin::views::{Body, Button, TreeItem, View, ViewEvent};
use openidea_plugin::{Error, Invocation, PickItem, host};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub const VIEW: &str = "model";
/// The command the server's lens on an Eloquent model class carries: the class name.
pub const LENS_COMMAND: &str = "laravel.modelView";

/// The column types Add Columns accepts (Laravel's `Blueprint` methods).
const COLUMN_TYPES: [&str; 14] = [
    "string",
    "text",
    "integer",
    "bigInteger",
    "unsignedBigInteger",
    "foreignId",
    "boolean",
    "decimal",
    "float",
    "date",
    "dateTime",
    "timestamp",
    "json",
    "uuid",
];

#[derive(Deserialize)]
struct Column {
    name: String,
    ty: String,
    nullable: bool,
    cast: Option<String>,
    fillable: bool,
    hidden: bool,
    location: Option<Place>,
}

#[derive(Deserialize)]
struct Relation {
    name: String,
    kind: String,
    related: Option<String>,
    location: Option<Place>,
}

/// A local scope or an accessor.
#[derive(Deserialize)]
struct Member {
    name: String,
    ty: Option<String>,
    location: Option<Place>,
}

#[derive(Deserialize)]
struct Related {
    kind: String,
    name: String,
    location: Place,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Model {
    class: String,
    location: Option<Place>,
    table: String,
    primary_key: String,
    key_type: String,
    schema_certain: bool,
    columns: Vec<Column>,
    relations: Vec<Relation>,
    scopes: Vec<Member>,
    accessors: Vec<Member>,
    related: Vec<Related>,
}

/// A column Add Columns adds.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct NewColumn {
    name: String,
    ty: String,
    nullable: bool,
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The columns of `title:string, body:text?, votes:integer`: a name, a type (`string`
/// when left out) and `?` for nullable.
pub fn parse_columns(typed: &str) -> Result<Vec<NewColumn>, Error> {
    let mut columns = Vec::new();
    for part in typed.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (part, nullable) = match part.strip_suffix('?') {
            Some(rest) => (rest.trim(), true),
            None => (part, false),
        };
        let (name, ty) = match part.split_once(':') {
            Some((name, ty)) => (name.trim(), ty.trim()),
            None => (part, "string"),
        };
        if !is_identifier(name) {
            return Err(Error::Invalid(format!("'{name}' is not a column name")));
        }
        if !COLUMN_TYPES.contains(&ty) {
            return Err(Error::Invalid(format!("'{ty}' is not a column type: use one of {}", COLUMN_TYPES.join(", "))));
        }
        columns.push(NewColumn { name: name.to_owned(), ty: ty.to_owned(), nullable });
    }
    if columns.is_empty() {
        return Err(Error::Invalid("Name at least one column, e.g. title:string, body:text?".into()));
    }
    Ok(columns)
}

fn storage_key(ctx: &Invocation) -> Result<String, Error> {
    Ok(format!("model:{}", server::project(ctx)?.root()?))
}

/// The class the project's model view shows.
fn shown(ctx: &Invocation) -> Result<Option<String>, Error> {
    host::storage::get(&storage_key(ctx)?)
}

/// Shows `class` in the model view of the invoking project.
fn show(ctx: &Invocation, class: &str) -> Result<(), Error> {
    host::storage::set(&storage_key(ctx)?, class)?;
    host::refresh_view(VIEW);
    host::reveal_view(VIEW)
}

/// The lens on an Eloquent model class was clicked: its argument is the class name.
pub fn lens(ctx: &Invocation, arg: Option<&str>) -> Result<(), Error> {
    let arguments = server::command_arguments(arg, LENS_COMMAND)?;
    let class = arguments.first().and_then(|a| a.as_str()).ok_or_else(|| Error::Invalid(LENS_COMMAND.into()))?;
    show(ctx, class)
}

#[derive(Deserialize)]
struct Models {
    models: Vec<String>,
}

/// Show Eloquent Model…: pick one of the project's models.
pub fn pick(ctx: &Invocation) -> Result<(), Error> {
    let context: Option<Models> = server::request(ctx, "generationContext", json!({}))?;
    let models = context.ok_or_else(|| Error::Invalid("This is not a Laravel project".into()))?.models;
    if models.is_empty() {
        return Err("This project has no Eloquent models".into());
    }
    let items: Vec<PickItem> = models
        .iter()
        .map(|class| PickItem {
            key: class.clone(),
            label: server::short_name(class).to_owned(),
            description: Some(class.clone()),
        })
        .collect();
    match host::pick("Show Eloquent Model", &items)? {
        Some(class) => show(ctx, &class),
        None => Ok(()),
    }
}

fn model(ctx: &Invocation, class: &str) -> Result<Model, Error> {
    server::request(ctx, "modelView", json!({ "class": class }))
}

/// A group's node and its members under it.
fn group(items: &mut Vec<TreeItem>, id: &str, title: &str, members: Vec<(String, String, Option<&Place>)>) {
    if members.is_empty() {
        return;
    }
    let key = format!("group:{id}");
    items.push(TreeItem::new(&key, &format!("{title} ({})", members.len())).expandable(true));
    for (index, (label, detail, place)) in members.into_iter().enumerate() {
        // Members can share a place (`$table->timestamps()` declares two columns), and one
        // without a declaration (a column only the database has) still needs a key.
        let item_key = match place {
            Some(place) => format!("{}:{id}", Target::nth(&place.path, place.range.start, index)),
            None => format!("{id}:{index}"),
        };
        let mut item = TreeItem::new(&item_key, &label).parent(&key);
        if !detail.is_empty() {
            item = item.detail(&detail);
        }
        items.push(item);
    }
}

pub fn render(ctx: &Invocation) -> Result<View, Error> {
    let Some(class) = shown(ctx)? else {
        return Ok(View::message(
            "Click the lens on an Eloquent model class, or run Laravel: Show Eloquent Model…, to see its table, \
             columns and relations here.",
        ));
    };
    let model = model(ctx, &class)?;
    let mut items = Vec::new();
    let columns = model
        .columns
        .iter()
        .map(|c| {
            let mut detail = format!("{}{}", if c.nullable { "?" } else { "" }, c.ty);
            let notes = [
                c.cast.as_ref().map(|cast| format!("cast {cast}")),
                c.fillable.then(|| "fillable".into()),
                c.hidden.then(|| "hidden".into()),
            ];
            for note in notes.into_iter().flatten() {
                detail.push_str(" · ");
                detail.push_str(&note);
            }
            (c.name.clone(), detail, c.location.as_ref())
        })
        .collect();
    group(&mut items, "columns", "Columns", columns);
    let relations = model
        .relations
        .iter()
        .map(|r| {
            let related = r.related.as_deref().map(server::short_name).unwrap_or_default();
            (r.name.clone(), format!("{} {related}", r.kind).trim().to_owned(), r.location.as_ref())
        })
        .collect();
    group(&mut items, "relations", "Relations", relations);
    fn members(members: &[Member]) -> Vec<(String, String, Option<&Place>)> {
        members.iter().map(|m| (m.name.clone(), m.ty.clone().unwrap_or_default(), m.location.as_ref())).collect()
    }
    group(&mut items, "scopes", "Scopes", members(&model.scopes));
    group(&mut items, "accessors", "Accessors", members(&model.accessors));
    let related = model
        .related
        .iter()
        .map(|r| (server::short_name(&r.name).to_owned(), r.kind.clone(), Some(&r.location)))
        .collect();
    group(&mut items, "related", "Related classes", related);

    let mut status = format!(
        "{} · table {} · key {} ({})",
        server::short_name(&model.class),
        model.table,
        model.primary_key,
        model.key_type
    );
    if !model.schema_certain {
        status.push_str(" · no migration creates the table: the columns are a guess");
    }
    let mut toolbar = vec![Button::new("add-columns", "Add Columns…"), Button::new("refresh", "Refresh")];
    if model.location.is_some() {
        toolbar.insert(0, Button::new("open", "Go to Class"));
    }
    let body = if items.is_empty() {
        Body::Message("Nothing is known about this model yet".into())
    } else {
        Body::Tree(items)
    };
    Ok(View { toolbar, body, status: Some(status) })
}

/// Add Columns…: asks for the columns, has the server write a migration (and add them
/// to `$fillable` when the user wants), and shows the migration.
fn add_columns(ctx: &Invocation, class: &str) -> Result<(), Error> {
    let prompt = "Columns to add, e.g. title:string, body:text?, votes:integer";
    let Some(typed) = host::input(prompt, None)? else { return Ok(()) };
    let columns = parse_columns(&typed)?;
    let fillable =
        host::confirm("Add Columns", &format!("Also add them to $fillable of {}?", server::short_name(class)))?;
    let applied: Applied =
        server::request(ctx, "addColumns", json!({ "class": class, "columns": columns, "fillable": fillable }))?;
    edit::apply_and_open(&server::project(ctx)?, &applied)?;
    host::refresh_view(VIEW);
    Ok(())
}

pub fn handle(event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
    match event {
        ViewEvent::Activate(key) => server::open(&key),
        ViewEvent::Button(id) => {
            let Some(class) = shown(ctx)? else { return Ok(()) };
            match id.as_str() {
                "add-columns" => add_columns(ctx, &class),
                "open" => match model(ctx, &class)?.location {
                    Some(place) => server::open(&Target::of(&place)),
                    None => Ok(()),
                },
                _ => {
                    host::refresh_view(VIEW);
                    Ok(())
                }
            }
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::mock;

    const MODEL: &str = r#"{
        "class":"App\\Models\\Post","name":"Post",
        "location":{"path":"/project/app/Models/Post.php","range":{"start":{"line":6,"col":6},"end":{"line":6,"col":10}}},
        "table":"posts","primaryKey":"id","keyType":"int","incrementing":true,"schemaCertain":true,
        "columns":[
            {"name":"title","ty":"string","nullable":false,"cast":null,"source":"migration","database":null,
             "fillable":true,"hidden":false,
             "location":{"path":"/project/database/migrations/2024_create_posts.php","range":{"start":{"line":14,"col":12},"end":{"line":14,"col":30}}}},
            {"name":"meta","ty":"string","nullable":true,"cast":"array","source":"database","database":"TEXT",
             "fillable":false,"hidden":true,"location":null}
        ],
        "relations":[{"name":"author","kind":"BelongsTo","related":"App\\Models\\User","many":false,
             "location":{"path":"/project/app/Models/Post.php","range":{"start":{"line":20,"col":20},"end":{"line":20,"col":26}}}}],
        "scopes":[],"accessors":[],
        "related":[{"kind":"factory","name":"PostFactory",
             "location":{"path":"/project/database/factories/PostFactory.php","range":{"start":{"line":8,"col":6},"end":{"line":8,"col":17}}}}]
    }"#;

    #[test]
    fn columns_are_typed_as_name_type_and_a_question_mark() {
        let column = |name: &str, ty: &str, nullable| NewColumn { name: name.into(), ty: ty.into(), nullable };
        assert_eq!(
            parse_columns("title:string, body : text?, slug, votes:integer ?,").unwrap(),
            [
                column("title", "string", false),
                column("body", "text", true),
                column("slug", "string", false),
                column("votes", "integer", true)
            ]
        );
        assert!(matches!(parse_columns("2fast:string"), Err(Error::Invalid(m)) if m.contains("column name")));
        assert!(matches!(parse_columns("title:varchar"), Err(Error::Invalid(m)) if m.contains("column type")));
        assert!(parse_columns(" , ").is_err());
    }

    #[test]
    fn the_lens_shows_the_model_in_the_view_of_its_project() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/modelView", MODEL);
        let Body::Message(hint) = render(&mock.invocation()).unwrap().body else { panic!("a hint before any model") };
        assert!(hint.contains("lens"));

        lens(&mock.invocation(), Some(r#"["App\\Models\\Post"]"#)).unwrap();
        assert_eq!(mock.storage()["model:/project"], "App\\Models\\Post");
        assert_eq!((mock.refreshed_views(), mock.revealed_views()), (vec![VIEW.to_owned()], vec![VIEW.to_owned()]));

        let view = render(&mock.invocation()).unwrap();
        assert_eq!(mock.language_server_requests()[0].params.as_deref(), Some(r#"{"class":"App\\Models\\Post"}"#));
        assert_eq!(view.status.as_deref(), Some("Post · table posts · key id (int)"));
        assert_eq!(view.toolbar.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["open", "add-columns", "refresh"]);
        let Body::Tree(items) = view.body else { panic!("a tree") };
        let shown: Vec<_> = items.iter().map(|i| (i.label.as_str(), i.detail.as_deref().unwrap_or(""))).collect();
        assert_eq!(
            shown,
            [
                ("Columns (2)", ""),
                ("title", "string · fillable"),
                ("meta", "?string · cast array · hidden"),
                ("Relations (1)", ""),
                ("author", "BelongsTo User"),
                ("Related classes (1)", ""),
                ("PostFactory", "factory"),
            ]
        );

        // An item goes to its declaration; a column only the database has goes nowhere.
        handle(ViewEvent::Activate(items[1].key.clone()), &mock.invocation()).unwrap();
        handle(ViewEvent::Activate(items[2].key.clone()), &mock.invocation()).unwrap();
        handle(ViewEvent::Button("open".into()), &mock.invocation()).unwrap();
        assert_eq!(
            mock.opened_locations(),
            [
                ("/project/database/migrations/2024_create_posts.php".to_owned(), 14, 12),
                ("/project/app/Models/Post.php".to_owned(), 6, 6)
            ]
        );

        // Another project's view shows its own model, or none.
        mock.set_project_root(Some("/other"));
        assert!(matches!(render(&mock.invocation()).unwrap().body, Body::Message(_)));
    }

    #[test]
    fn add_columns_writes_the_servers_migration() {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/modelView", MODEL);
        mock.set_language_server_response(
            server::SERVER,
            "phoxite/laravel/addColumns",
            r#"{"edit":{"documentChanges":[
                {"kind":"create","uri":"file:///project/database/migrations/2024_add_votes.php"},
                {"textDocument":{"uri":"file:///project/database/migrations/2024_add_votes.php","version":null},
                 "edits":[{"range":{"start":{"line":0,"character":0},"end":{"line":0,"character":0}},"newText":"<?php // votes\n"}]}
            ]},"created":["file:///project/database/migrations/2024_add_votes.php"]}"#,
        );
        lens(&mock.invocation(), Some(r#"["App\\Models\\Post"]"#)).unwrap();
        mock.answer_next_prompt(Some(&["votes:integer?"]));
        mock.answer_next_prompt(Some(&[]));
        handle(ViewEvent::Button("add-columns".into()), &mock.invocation()).unwrap();

        let sent = mock.language_server_requests();
        let params: serde_json::Value = serde_json::from_str(sent.last().unwrap().params.as_deref().unwrap()).unwrap();
        assert_eq!(
            params,
            json!({ "class": "App\\Models\\Post", "columns": [{ "name": "votes", "ty": "integer", "nullable": true }], "fillable": true })
        );
        let project = mock.invocation().project().unwrap();
        assert_eq!(project.read_file("/project/database/migrations/2024_add_votes.php").unwrap(), "<?php // votes\n");
        assert_eq!(mock.opened_locations(), [("/project/database/migrations/2024_add_votes.php".to_owned(), 0, 0)]);
    }
}
