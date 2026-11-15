//! Laravel tools for OpenIDEA, on phoxite-lsp's `phoxite/laravel/*` requests
//! (`docs/editors.md`). The plugin keeps no state: each view and command asks the
//! server of its invoking project, so windows never mix.

use openidea_plugin::views::{Body, TreeItem, View, ViewEvent, Viewport, Views};
use openidea_plugin::{Error, Invocation, PickItem, Plugin, host, register_plugin};
use serde::{Deserialize, Serialize};

/// The server's name in OpenIDEA's `languages/php.toml`.
const SERVER: &str = "phoxite";
const ROUTES: &str = "phoxite/laravel/routes";
const ROUTES_VIEW: &str = "routes";

/// A route as the server describes it.
#[derive(Deserialize)]
struct Route {
    name: Option<String>,
    methods: Vec<String>,
    uri: String,
    action: String,
    path: String,
    range: Range,
}

#[derive(Deserialize)]
struct Range {
    start: Place,
}

/// A place in a file: 0-based line and UTF-16 column.
#[derive(Deserialize, Serialize, Clone, Copy)]
struct Place {
    line: u32,
    col: u32,
}

/// Where a tree item or a pick leads: its key, as JSON.
#[derive(Deserialize, Serialize)]
struct Target {
    path: String,
    at: Place,
}

impl Route {
    /// `GET|HEAD /posts/{post}`.
    fn label(&self) -> String {
        let uri = if self.uri.starts_with('/') { self.uri.clone() } else { format!("/{}", self.uri) };
        format!("{} {uri}", self.methods.join("|"))
    }

    /// The route's name when it has one, and what handles it.
    fn detail(&self) -> String {
        match &self.name {
            Some(name) if !name.is_empty() => format!("{name} · {}", self.action),
            _ => self.action.clone(),
        }
    }

    fn key(&self) -> String {
        serde_json::to_string(&Target { path: self.path.clone(), at: self.range.start }).unwrap_or_default()
    }
}

/// The project's routes matching every term of `query` (all when empty).
fn routes(ctx: &Invocation, query: &str) -> Result<Vec<Route>, Error> {
    let project = ctx.project().ok_or("Open a Laravel project first")?;
    let params = serde_json::json!({ "query": query }).to_string();
    let answer = project.language_server_request(SERVER, ROUTES, Some(&params)).map_err(|e| match e {
        Error::NotFound(_) => Error::NotFound(
            "Phoxite is not running for this project: choose it for PHP in Settings › Languages & Frameworks › \
             Language Servers"
                .into(),
        ),
        e => e,
    })?;
    serde_json::from_str(&answer).map_err(|e| Error::Failed(format!("{ROUTES}: {e}")))
}

/// `path` as the view shows it: relative to the project.
fn relative<'a>(path: &'a str, root: Option<&str>) -> &'a str {
    root.and_then(|root| path.strip_prefix(root))
        .map(|rest| rest.trim_start_matches(['/', '\\']))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(path)
}

fn open(key: &str) -> Result<(), Error> {
    let Ok(target) = serde_json::from_str::<Target>(key) else { return Ok(()) }; // a route file's node
    host::open_location(&target.path, target.at.line, target.at.col)
}

#[derive(Default)]
pub struct Laravel;

impl Plugin for Laravel {
    fn run_command(&mut self, id: &str, ctx: &Invocation, _arg: Option<&str>) -> Result<Option<String>, Error> {
        match id {
            "go-to-route" => {
                let items: Vec<PickItem> = routes(ctx, "")?
                    .iter()
                    .map(|r| PickItem { key: r.key(), label: r.label(), description: Some(r.detail()) })
                    .collect();
                if items.is_empty() {
                    return Err("This project has no routes".into());
                }
                if let Some(key) = host::pick("Go to Route", &items)? {
                    open(&key)?;
                }
                Ok(None)
            }
            "refresh-routes" => {
                host::refresh_view(ROUTES_VIEW);
                Ok(None)
            }
            _ => Err(Error::NotFound(id.into())),
        }
    }
}

impl Views for Laravel {
    /// The routes by route file, in the server's order.
    fn render(&mut self, _view: &str, _viewport: Option<Viewport>, ctx: &Invocation) -> Result<View, Error> {
        let routes = routes(ctx, "")?;
        if routes.is_empty() {
            return Ok(View { toolbar: vec![], body: Body::Message("No routes in this project".into()), status: None });
        }
        let root = ctx.project().and_then(|p| p.root().ok());
        let mut items: Vec<TreeItem> = Vec::new();
        let mut files: Vec<&str> = Vec::new();
        for route in &routes {
            let file = format!("file:{}", route.path);
            if !files.contains(&route.path.as_str()) {
                files.push(&route.path);
                items.push(TreeItem::new(&file, relative(&route.path, root.as_deref())).expandable(true));
            }
            items.push(TreeItem::new(&route.key(), &route.label()).parent(&file).detail(&route.detail()));
        }
        let status = format!("{} routes in {} files", routes.len(), files.len());
        Ok(View { toolbar: vec![], body: Body::Tree(items), status: Some(status) })
    }

    fn handle(&mut self, _view: &str, event: ViewEvent, _ctx: &Invocation) -> Result<(), Error> {
        match event {
            ViewEvent::Activate(key) => open(&key),
            _ => Ok(()),
        }
    }
}

register_plugin!(Laravel, views);

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    const ANSWER: &str = r#"[
        {"name":"posts.index","methods":["GET","HEAD"],"uri":"posts","action":"PostController@index",
         "middleware":["web"],"path":"/project/routes/web.php","range":{"start":{"line":4,"col":0},"end":{"line":4,"col":40}},
         "url":"http://localhost/posts","request":"GET http://localhost/posts"},
        {"name":null,"methods":["POST"],"uri":"/api/posts","action":"Closure",
         "middleware":[],"path":"/project/routes/api.php","range":{"start":{"line":9,"col":2},"end":{"line":9,"col":30}},
         "url":"http://localhost/api/posts","request":"POST http://localhost/api/posts"}
    ]"#;

    fn mock() -> MockHost {
        let mock = MockHost::install().with_capabilities(&["lsp:phoxite", "project.read"]);
        mock.set_project_root(Some("/project"));
        mock.set_language_server_response(SERVER, ROUTES, ANSWER);
        mock
    }

    #[test]
    fn the_routes_view_lists_the_servers_routes_by_file_and_opens_them() {
        let mock = mock();
        let view = Laravel.render(ROUTES_VIEW, None, &mock.invocation()).unwrap();
        let Body::Tree(items) = view.body else { panic!("a tree") };
        let shown: Vec<_> = items.iter().map(|i| (i.label.as_str(), i.detail.as_deref())).collect();
        assert_eq!(
            shown,
            [
                ("routes/web.php", None),
                ("GET|HEAD /posts", Some("posts.index · PostController@index")),
                ("routes/api.php", None),
                ("POST /api/posts", Some("Closure")),
            ]
        );
        assert_eq!(items[1].parent.as_deref(), Some("file:/project/routes/web.php"));
        assert_eq!(view.status.as_deref(), Some("2 routes in 2 files"));
        assert_eq!(mock.language_server_requests()[0].params.as_deref(), Some(r#"{"query":""}"#));

        Laravel.handle(ROUTES_VIEW, ViewEvent::Activate(items[3].key.clone()), &mock.invocation()).unwrap();
        Laravel.handle(ROUTES_VIEW, ViewEvent::Activate(items[0].key.clone()), &mock.invocation()).unwrap();
        assert_eq!(mock.opened_locations(), [("/project/routes/api.php".to_owned(), 9, 2)]);
    }

    #[test]
    fn go_to_route_opens_the_chosen_route() {
        let mock = mock();
        mock.answer_next_prompt(Some(&[r#"{"path":"/project/routes/web.php","at":{"line":4,"col":0}}"#]));
        Laravel.run_command("go-to-route", &mock.invocation(), None).unwrap();
        assert_eq!(mock.opened_locations(), [("/project/routes/web.php".to_owned(), 4, 0)]);
    }

    #[test]
    fn a_server_that_is_not_running_says_how_to_choose_it() {
        let mock = MockHost::install().with_capabilities(&["lsp:phoxite"]);
        mock.set_project_root(Some("/project"));
        let Err(Error::NotFound(message)) = Laravel.render(ROUTES_VIEW, None, &mock.invocation()) else {
            panic!("not-found")
        };
        assert!(message.contains("Language Servers"), "{message}");
    }

    #[test]
    fn without_the_grant_the_request_is_denied() {
        let mock = mock().with_capabilities(&[]);
        assert!(
            matches!(Laravel.run_command("go-to-route", &mock.invocation(), None), Err(Error::Denied(c)) if c == "lsp:phoxite")
        );
    }
}
