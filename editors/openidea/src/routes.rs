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
    /// The URL with the application's `APP_URL`.
    #[serde(default)]
    url: String,
    /// An HTTP request for the route, in `.http` file syntax.
    #[serde(default)]
    request: String,
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

/// One of the project's routes, picked by the user; none when they gave up.
fn pick_one(ctx: &Invocation, title: &str) -> Result<Option<Route>, Error> {
    let mut routes = routes(ctx, "")?;
    if routes.is_empty() {
        return Err("This project has no routes".into());
    }
    // By position: several routes can share a place, and a URL.
    let items: Vec<PickItem> = routes
        .iter()
        .enumerate()
        .map(|(index, r)| PickItem { key: index.to_string(), label: r.label(), description: Some(r.detail()) })
        .collect();
    let chosen = host::pick(title, &items)?.and_then(|key| key.parse::<usize>().ok());
    Ok(chosen.filter(|index| *index < routes.len()).map(|index| routes.swap_remove(index)))
}

/// Copy Route URL…: the URL of a route, with the application's `APP_URL`.
pub fn copy_url(ctx: &Invocation) -> Result<(), Error> {
    match pick_one(ctx, "Copy Route URL")? {
        Some(route) => host::copy_to_clipboard(&route.url),
        None => Ok(()),
    }
}

/// Copy Route as HTTP Request…: the route as a request of a `.http` file.
pub fn copy_request(ctx: &Invocation) -> Result<(), Error> {
    match pick_one(ctx, "Copy Route as HTTP Request")? {
        Some(route) => host::copy_to_clipboard(&route.request),
        None => Ok(()),
    }
}

/// Open Route in Browser…: a route without parameters opens as it is; one with
/// parameters (`/posts/{post}`) has nothing to put in them, and says so.
pub fn open_in_browser(ctx: &Invocation) -> Result<(), Error> {
    let Some(route) = pick_one(ctx, "Open Route in Browser")? else { return Ok(()) };
    if route.url.contains('{') {
        return Err(Error::Invalid(format!("{} has parameters: copy its URL and fill them in", route.label())));
    }
    host::open_url(&route.url)
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
    fn a_routes_url_is_copied_or_opened_in_the_browser() {
        let mock = with_routes();
        mock.answer_next_prompt(Some(&["1"]));
        copy_url(&mock.invocation()).unwrap();
        assert_eq!(mock.clipboard().as_deref(), Some("http://localhost/api/posts"));
        mock.answer_next_prompt(Some(&["0"]));
        copy_request(&mock.invocation()).unwrap();
        assert_eq!(mock.clipboard().as_deref(), Some("GET http://localhost/posts"));
        mock.answer_next_prompt(Some(&["0"]));
        open_in_browser(&mock.invocation()).unwrap();
        assert_eq!(mock.opened_urls(), ["http://localhost/posts"]);
        // Dismissed: nothing more is copied or opened.
        open_in_browser(&mock.invocation()).unwrap();
        assert_eq!(mock.opened_urls().len(), 1);
    }

    #[test]
    fn a_route_with_parameters_is_not_opened_as_it_is() {
        let mock = mock();
        let answer = ANSWER.replace("http://localhost/posts\"", "http://localhost/posts/{post}\"");
        mock.set_language_server_response(server::SERVER, "phoxite/laravel/routes", &answer);
        mock.answer_next_prompt(Some(&["0"]));
        assert!(matches!(open_in_browser(&mock.invocation()), Err(Error::Invalid(m)) if m.contains("parameters")));
        assert!(mock.opened_urls().is_empty());
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
