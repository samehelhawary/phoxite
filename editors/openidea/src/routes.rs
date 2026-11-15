//! The routes: the Laravel Routes view, Go to Route and the route lenses.

use crate::server::{self, Range, Target};
use openidea_plugin::views::{Body, TreeItem, View};
use openidea_plugin::{Error, Invocation, PickItem, host};
use serde::Deserialize;
use serde_json::json;

pub const VIEW: &str = "routes";
/// The command the server's route lenses and URI hints carry.
pub const LENS_COMMAND: &str = "laravel.routeActions";

/// A route as the server describes it.
#[derive(Deserialize)]
pub struct Route {
    name: Option<String>,
    methods: Vec<String>,
    uri: String,
    action: String,
    path: String,
    range: Range,
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
        Target::key(&self.path, self.range.start)
    }
}

/// The project's routes matching every term of `query` (all when empty).
fn routes(ctx: &Invocation, query: &str) -> Result<Vec<Route>, Error> {
    server::request(ctx, "routes", json!({ "query": query }))
}

fn pick(routes: &[Route]) -> Result<(), Error> {
    let items: Vec<PickItem> =
        routes.iter().map(|r| PickItem { key: r.key(), label: r.label(), description: Some(r.detail()) }).collect();
    if let Some(key) = host::pick("Go to Route", &items)? {
        server::open(&key)?;
    }
    Ok(())
}

/// Go to Route…: a picker over every route.
pub fn go_to_route(ctx: &Invocation) -> Result<(), Error> {
    let routes = routes(ctx, "")?;
    if routes.is_empty() {
        return Err("This project has no routes".into());
    }
    pick(&routes)
}

/// A route lens or URI hint was clicked: the first argument is the routes as a JSON
/// string. One route opens; several ask which.
pub fn lens(arg: Option<&str>) -> Result<(), Error> {
    let arguments = server::command_arguments(arg, LENS_COMMAND)?;
    let text = arguments.first().and_then(|a| a.as_str()).unwrap_or("[]");
    let routes: Vec<Route> = serde_json::from_str(text).map_err(|e| Error::Invalid(format!("{LENS_COMMAND}: {e}")))?;
    match routes.as_slice() {
        [] => Ok(()),
        [only] => server::open(&only.key()),
        several => pick(several),
    }
}

/// The routes by route file, in the server's order.
pub fn render(ctx: &Invocation) -> Result<View, Error> {
    let routes = routes(ctx, "")?;
    if routes.is_empty() {
        return Ok(View::message("No routes in this project"));
    }
    let root = ctx.project().and_then(|p| p.root().ok());
    let mut items: Vec<TreeItem> = Vec::new();
    let mut files: Vec<&str> = Vec::new();
    for (index, route) in routes.iter().enumerate() {
        let file = format!("file:{}", route.path);
        if !files.contains(&route.path.as_str()) {
            files.push(&route.path);
            items.push(TreeItem::new(&file, &server::relative(&route.path, root.as_deref())).expandable(true));
        }
        // The routes of one `Route::resource` are at one place: the index keeps the keys apart.
        let key = Target::nth(&route.path, route.range.start, index);
        items.push(TreeItem::new(&key, &route.label()).parent(&file).detail(&route.detail()));
    }
    let status = format!("{} routes in {} files", routes.len(), files.len());
    Ok(View { toolbar: vec![], body: Body::Tree(items), status: Some(status) })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::tests::mock;
    use openidea_plugin::mock::MockHost;
    use serde_json::Value;

    pub const ANSWER: &str = r#"[
        {"name":"posts.index","methods":["GET","HEAD"],"uri":"posts","action":"PostController@index",
         "middleware":["web"],"path":"/project/routes/web.php","range":{"start":{"line":4,"col":0},"end":{"line":4,"col":40}},
         "url":"http://localhost/posts","request":"GET http://localhost/posts"},
        {"name":null,"methods":["POST"],"uri":"/api/posts","action":"Closure",
         "middleware":[],"path":"/project/routes/api.php","range":{"start":{"line":9,"col":2},"end":{"line":9,"col":30}},
         "url":"http://localhost/api/posts","request":"POST http://localhost/api/posts"}
    ]"#;

    fn with_routes() -> MockHost {
        let mock = mock();
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/routes", ANSWER);
        mock
    }

    #[test]
    fn the_routes_view_lists_the_servers_routes_by_file() {
        let mock = with_routes();
        let view = render(&mock.invocation()).unwrap();
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

        server::open(&items[3].key).unwrap();
        server::open(&items[0].key).unwrap();
        assert_eq!(mock.opened_locations(), [("/project/routes/api.php".to_owned(), 9, 2)]);
    }

    #[test]
    fn go_to_route_opens_the_chosen_route() {
        let mock = with_routes();
        mock.answer_next_prompt(Some(&[r#"{"path":"/project/routes/web.php","at":{"line":4,"col":0}}"#]));
        go_to_route(&mock.invocation()).unwrap();
        assert_eq!(mock.opened_locations(), [("/project/routes/web.php".to_owned(), 4, 0)]);
    }

    #[test]
    fn a_route_lens_goes_to_its_route_or_asks_which() {
        let mock = with_routes();
        let routes: Vec<Value> = serde_json::from_str(ANSWER).unwrap();
        let lens_arg = |routes: &[Value]| json!([serde_json::to_string(routes).unwrap()]).to_string();

        // One route: straight to it.
        lens(Some(&lens_arg(&routes[..1]))).unwrap();
        assert_eq!(mock.opened_locations(), [("/project/routes/web.php".to_owned(), 4, 0)]);

        // Several: the user picks.
        mock.answer_next_prompt(Some(&[r#"{"path":"/project/routes/api.php","at":{"line":9,"col":2}}"#]));
        lens(Some(&lens_arg(&routes))).unwrap();
        assert_eq!(mock.opened_locations().last().unwrap(), &("/project/routes/api.php".to_owned(), 9, 2));

        // The lens needs no request to the server, and arguments that are not routes are refused.
        assert!(mock.language_server_requests().is_empty());
        assert!(matches!(lens(Some("{")), Err(Error::Invalid(_))));
    }
}
