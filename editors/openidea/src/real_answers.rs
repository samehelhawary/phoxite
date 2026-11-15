//! The plugin against what a real phoxite-lsp answered on `fixtures/laravel-app`
//! (`tests/fixtures/*.json`, written by `tests/capture.mjs`): the other tests use
//! answers typed by hand, which only prove the plugin agrees with itself.

use crate::tests::mock;
use crate::{Laravel, model, routes, server};
use openidea_plugin::mock::MockHost;
use openidea_plugin::views::{Body, ViewEvent, Views};
use openidea_plugin::{Plugin, Project};

fn answer(mock: &MockHost, name: &str, json: &str) {
    mock.set_language_server_response(server::SERVER, &format!("phoxite/laravel/{name}"), json);
}

/// A tree's keys must differ, or the IDE mixes its rows up: the fixture has columns that
/// share a declaration (`$table->timestamps()`), which hand-written answers did not.
fn assert_keys_differ(items: &[openidea_plugin::views::TreeItem]) {
    let mut keys: Vec<&str> = items.iter().map(|i| i.key.as_str()).collect();
    keys.sort_unstable();
    let before = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), before, "two items of the tree have the same key");
}

fn project(mock: &MockHost) -> Project {
    mock.invocation().project().unwrap()
}

#[test]
fn the_routes_view_shows_the_fixtures_routes() {
    let mock = mock();
    answer(&mock, "routes", include_str!("../tests/fixtures/routes.json"));
    let view = Laravel.render(routes::VIEW, None, &mock.invocation()).unwrap();
    let Body::Tree(items) = view.body else { panic!("a tree") };
    let labels: Vec<_> = items.iter().map(|i| i.label.as_str()).collect();
    assert_eq!(&labels[..3], ["routes/web.php", "GET /", "GET /about"]);
    assert_keys_differ(&items);
    Laravel.handle(routes::VIEW, ViewEvent::Activate(items[1].key.clone()), &mock.invocation()).unwrap();
    assert_eq!(mock.opened_locations(), [("/project/routes/web.php".to_owned(), 6, 7)]);
}

#[test]
fn the_model_view_shows_the_fixtures_post_model() {
    let mock = mock();
    answer(&mock, "modelView", include_str!("../tests/fixtures/modelView.json"));
    Laravel.run_command("model-view", &mock.invocation(), Some(r#"["App\\Models\\Post"]"#)).unwrap();
    let view = Laravel.render(model::VIEW, None, &mock.invocation()).unwrap();
    assert_eq!(view.status.as_deref(), Some("Post · table posts · key id (int)"));
    let Body::Tree(items) = view.body else { panic!("a tree") };
    assert_keys_differ(&items);
    assert!(items.iter().any(|i| i.label == "PostFactory" && i.detail.as_deref() == Some("factory")));
    let groups: Vec<_> = items.iter().filter(|i| i.parent.is_none()).map(|i| i.label.as_str()).collect();
    assert_eq!(groups, ["Columns (12)", "Relations (3)", "Scopes (1)", "Accessors (1)", "Related classes (4)"]);
    assert!(items.iter().any(|i| i.label == "author" && i.detail.as_deref() == Some("BelongsTo User")));
    assert!(items.iter().any(|i| i.label == "title" && i.detail.as_deref().is_some_and(|d| d.contains("string"))));
}

#[test]
fn artisan_runs_a_command_of_the_fixture() {
    let mock = mock();
    answer(&mock, "artisanCommands", include_str!("../tests/fixtures/artisanCommands.json"));
    mock.answer_next_prompt(Some(&["--dry-run"]));
    Laravel.run_command("run-artisan-lens", &mock.invocation(), Some(r#"["posts:publish"]"#)).unwrap();
    assert_eq!(mock.terminals()[0].args, ["artisan", "posts:publish", "--dry-run"]);
}

#[test]
fn a_generator_of_the_fixture_writes_the_servers_class() {
    let mock = mock();
    answer(&mock, "generationContext", include_str!("../tests/fixtures/generationContext.json"));
    answer(&mock, "generateClass", include_str!("../tests/fixtures/generateClass.json"));
    mock.answer_next_prompt(Some(&["controller"]));
    mock.answer_next_prompt(Some(&["CapturedController"]));
    mock.answer_next_prompt(Some(&["App\\Models\\Post"]));
    Laravel.run_command("generate", &mock.invocation(), None).unwrap();
    let written = project(&mock).read_file("/project/app/Http/Controllers/CapturedController.php").unwrap();
    assert!(written.starts_with("<?php\n\nnamespace App\\Http\\Controllers;"), "{written}");
    assert!(written.contains("class CapturedController extends Controller"));
}

#[test]
fn add_columns_writes_the_migration_and_edits_the_model_in_place() {
    let mock = mock();
    let post = include_str!("../../../fixtures/laravel-app/app/Models/Post.php");
    mock.add_file("/project/app/Models/Post.php", post);
    answer(&mock, "modelView", include_str!("../tests/fixtures/modelView.json"));
    answer(&mock, "addColumns", include_str!("../tests/fixtures/addColumns.json"));
    Laravel.run_command("model-view", &mock.invocation(), Some(r#"["App\\Models\\Post"]"#)).unwrap();
    mock.answer_next_prompt(Some(&["captured_votes:integer?"]));
    mock.answer_next_prompt(Some(&[]));
    Laravel.handle(model::VIEW, ViewEvent::Button("add-columns".into()), &mock.invocation()).unwrap();

    let migration = "/project/database/migrations/2024_05_01_120000_add_captured_votes_to_posts_table.php";
    assert!(project(&mock).read_file(migration).unwrap().contains("captured_votes"));
    // The one edit in the model adds the column to `$fillable` and leaves the rest alone.
    let edited = project(&mock).read_file("/project/app/Models/Post.php").unwrap();
    assert!(edited.contains("'captured_votes'"), "{edited}");
    assert_eq!(edited.replace(", 'captured_votes'", ""), post.replace("\r\n", "\n"), "nothing else changed");
}

#[test]
fn a_new_model_writes_every_file_the_server_creates() {
    let mock = mock();
    answer(&mock, "newModel", include_str!("../tests/fixtures/newModel.json"));
    mock.answer_next_prompt(Some(&["CapturedThing"]));
    mock.answer_next_prompt(Some(&["title:string"]));
    mock.answer_next_prompt(Some(&["migration", "factory", "controller", "timestamps"]));
    Laravel.run_command("new-model", &mock.invocation(), None).unwrap();
    for (path, content) in [
        ("/project/app/Models/CapturedThing.php", "class CapturedThing"),
        ("/project/database/migrations/2024_05_01_120000_create_captured_things_table.php", "captured_things"),
        ("/project/database/factories/CapturedThingFactory.php", "CapturedThingFactory"),
        ("/project/app/Http/Controllers/CapturedThingController.php", "CapturedThingController"),
    ] {
        assert!(project(&mock).read_file(path).unwrap().contains(content), "{path}");
    }
    assert_eq!(mock.opened_locations(), [("/project/app/Models/CapturedThing.php".to_owned(), 0, 0)]);
}

#[test]
fn a_translation_key_creates_the_locales_file() {
    let mock = mock();
    answer(&mock, "createTranslation", include_str!("../tests/fixtures/createTranslation.json"));
    let request = serde_json::json!({ "key": "captured.greeting", "locales": [
        { "locale": "en", "text": "Greeting", "exists": true },
        { "locale": "fr", "text": "Greeting", "exists": false },
    ] });
    mock.answer_next_prompt(Some(&["Bonjour"]));
    let arg = serde_json::json!([request.to_string()]).to_string();
    Laravel.run_command("create-translation", &mock.invocation(), Some(&arg)).unwrap();
    let written = project(&mock).read_file("/project/lang/fr/captured.php").unwrap();
    assert!(written.contains("'greeting' => 'Bonjour'"), "{written}");
}
