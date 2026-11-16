//! Laravel tools for OpenIDEA, on phoxite-lsp's `phoxite/laravel/*` requests
//! (`docs/editors.md`). The server does the work; the plugin asks it and shows the
//! answers. It keeps nothing in memory between calls: each view and command asks the
//! server of its invoking project, so windows never mix.

mod artisan;
mod edit;
mod generate;
mod model;
mod routes;
mod server;
mod translation;

use openidea_plugin::views::{View, ViewEvent, Viewport, Views};
use openidea_plugin::{Error, Event, Invocation, Plugin, host, register_plugin};

#[derive(Default)]
pub struct Laravel;

impl Plugin for Laravel {
    fn run_command(&mut self, id: &str, ctx: &Invocation, arg: Option<&str>) -> Result<Option<String>, Error> {
        match id {
            "go-to-route" => routes::go_to_route(ctx)?,
            "refresh-routes" => host::refresh_view(routes::VIEW),
            "copy-route-url" => routes::copy_url(ctx)?,
            "copy-route-request" => routes::copy_request(ctx)?,
            "open-route" => routes::open_in_browser(ctx)?,
            "run-artisan" => artisan::pick_and_run(ctx)?,
            "show-model" => model::pick(ctx)?,
            "generate" => generate::generate(ctx)?,
            "new-model" => generate::new_model(ctx)?,
            // The commands the server's lenses and actions hand to the client
            // (`[[contributes.serverCommands]]`): `arg` is their `arguments` as JSON.
            "route-actions" => routes::lens(arg)?,
            "model-view" => model::lens(ctx, arg)?,
            "run-artisan-lens" => artisan::lens(ctx, arg)?,
            "create-translation" => translation::create(ctx, arg)?,
            _ => return Err(Error::NotFound(id.into())),
        }
        Ok(None)
    }

    /// Files of the project changed (a save, a generator, a checkout): what the views
    /// show comes from its PHP files, so both are read again.
    fn handle_events(&mut self, events: &[Event], _ctx: &Invocation) {
        let php = |paths: &[String]| paths.iter().any(|p| p.ends_with(".php"));
        let changed = events.iter().any(|event| match event {
            Event::FilesChanged(batch) => php(&batch.created) || php(&batch.modified) || php(&batch.removed),
            _ => false,
        });
        if changed {
            host::refresh_view(routes::VIEW);
            host::refresh_view(model::VIEW);
        }
    }
}

impl Views for Laravel {
    fn render(&mut self, view: &str, _viewport: Option<Viewport>, ctx: &Invocation) -> Result<View, Error> {
        match view {
            routes::VIEW => routes::render(ctx),
            model::VIEW => model::render(ctx),
            _ => Err(Error::NotFound(view.into())),
        }
    }

    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        match (view, event) {
            (model::VIEW, event) => model::handle(event, ctx),
            (_, ViewEvent::Activate(key)) => server::open(&key),
            _ => Ok(()),
        }
    }
}

register_plugin!(Laravel, views);

#[cfg(test)]
mod real_answers;

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    /// A host with the project `/project` open and what the manifest asks for granted.
    pub fn mock() -> MockHost {
        let mock = MockHost::install().with_capabilities(&[
            "lsp:phoxite",
            "project.read",
            "project.write",
            "editor.read",
            "editor.write",
            "storage",
            "events",
            "clipboard",
            "process:php",
        ]);
        mock.set_project_root(Some("/project"));
        mock
    }

    #[test]
    fn a_server_that_is_not_running_says_how_to_choose_it() {
        let mock = mock();
        let Err(Error::NotFound(message)) = Laravel.render(routes::VIEW, None, &mock.invocation()) else {
            panic!("not-found")
        };
        assert!(message.contains("Language Servers"), "{message}");
    }

    #[test]
    fn without_the_grant_the_request_is_denied() {
        let mock = mock().with_capabilities(&["project.read"]);
        assert!(
            matches!(Laravel.run_command("go-to-route", &mock.invocation(), None), Err(Error::Denied(c)) if c == "lsp:phoxite")
        );
    }

    #[test]
    fn the_views_are_read_again_when_php_files_of_the_project_change() {
        use openidea_plugin::FilesChanged;
        let mock = mock();
        let batch = |path: &str| {
            Event::FilesChanged(FilesChanged { created: vec![path.to_owned()], modified: vec![], removed: vec![] })
        };
        Laravel.handle_events(&[batch("/project/storage/logs/laravel.log")], &mock.invocation());
        assert!(mock.refreshed_views().is_empty(), "a log file changes nothing the views show");
        Laravel.handle_events(&[batch("/project/database/migrations/2030_add_votes.php")], &mock.invocation());
        assert_eq!(mock.refreshed_views(), [routes::VIEW, model::VIEW]);
        assert!(include_str!("../plugin.toml").contains("on-event:files-changed"));
    }

    #[test]
    fn every_command_of_the_manifest_is_one_the_plugin_runs() {
        let manifest = include_str!("../plugin.toml");
        let mock = mock();
        let mut commands = 0;
        for line in manifest.lines() {
            let id = line.strip_prefix("id = \"").and_then(|rest| rest.strip_suffix('"'));
            let run = line.strip_prefix("run = \"").and_then(|rest| rest.split('"').next());
            for id in id.into_iter().chain(run).filter(|id| !["phoxite.laravel", "routes", "model"].contains(id)) {
                commands += 1;
                let result = Laravel.run_command(id, &mock.invocation(), None);
                assert!(!matches!(&result, Err(Error::NotFound(m)) if m == id), "{id} is declared but not handled");
            }
        }
        assert!(commands >= 17, "only {commands} commands found in plugin.toml");
        for view in ["routes", "model"] {
            assert!(manifest.contains(&format!("id = \"{view}\"")), "{view}");
            assert!(!matches!(Laravel.render(view, None, &mock.invocation()), Err(Error::NotFound(m)) if m == view));
        }
    }
}
