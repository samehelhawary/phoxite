//! Completion, hover and goto for Doctrine's magic repository finders, on
//! the stubs of the Symfony provider's tests.

use super::*;
use crate::complete::{CompletionKind, Completions};
use crate::types::provider::symfony::tests::{APP, FRAMEWORK_BUNDLE, LIBRARIES};

const MAIN: &str = "bin/main.php";

/// `bin/main.php` importing the stubs' classes, then `body`.
fn main(body: &str) -> String {
    format!(
        "<?php\nnamespace Main;\n\nuse App\\Repository\\ProductRepository;\nuse Doctrine\\ORM\\EntityManagerInterface;\nuse App\\Entity\\Product;\n\n{body}\n"
    )
}

fn symfony(installed: bool) -> Project {
    let mut files = vec![("lib/libraries.php", LIBRARIES), ("lib/app.php", APP)];
    if installed {
        files.push(("lib/bundle.php", FRAMEWORK_BUNDLE));
    }
    Project::new(&files)
}

fn line_of(text: &str, needle: &str) -> u32 {
    text.lines().position(|line| line.contains(needle)).unwrap_or_else(|| panic!("{needle}")) as u32
}

impl Project {
    fn complete_symfony(&mut self, body: &str) -> Completions {
        let (text, pos) = caret(&main(body));
        let doc = self.open(MAIN, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found
    }

    fn hover_symfony(&mut self, body: &str) -> String {
        let (text, pos) = caret(&main(body));
        let doc = self.open(MAIN, &text);
        let hover = self.engine.hover(doc, pos);
        self.engine.document_closed(doc);
        hover.unwrap_or_else(|| panic!("a hover in {body}")).markdown
    }
}

fn has(found: &Completions, label: &str) -> bool {
    found.items.iter().any(|item| item.label == label)
}

#[test]
fn completion_lists_magic_finders_of_the_entity_fields() {
    let mut project = symfony(true);
    let found = project.complete_symfony("function f(ProductRepository $r) { $r->/*|*/ }");
    for label in ["findOneByName", "findById", "countByName", "cheapest", "findOneBy"] {
        assert!(has(&found, label), "{label}");
    }
    let finder = found.items.iter().find(|item| item.label == "findOneByName").expect("findOneByName");
    assert_eq!(finder.kind, CompletionKind::Method);

    // Through getRepository().
    let found = project
        .complete_symfony("function f(EntityManagerInterface $em) { $em->getRepository(Product::class)->/*|*/ }");
    assert!(has(&found, "findOneByName"));

    // Off without symfony/framework-bundle.
    let mut project = symfony(false);
    let found = project.complete_symfony("function f(ProductRepository $r) { $r->/*|*/ }");
    assert!(!has(&found, "findOneByName"));
    assert!(has(&found, "cheapest"));
}

#[test]
fn hover_and_goto_on_a_magic_finder() {
    let mut project = symfony(true);
    let text = project.hover_symfony("function f(ProductRepository $r) { $r->findOne/*|*/ByName('a'); }");
    assert!(text.contains("findOneBy"), "{text}");

    let body = "function f(ProductRepository $r) { $r->findOne/*|*/ByName('a'); }";
    let found = project.definition(MAIN, &main(body)).expect("handled");
    // `EntityRepository::findOneBy()`, where `__call` ends up.
    let line = line_of(LIBRARIES, "public function findOneBy(array $criteria) { return null; }");
    assert_eq!(found, [target("lib/libraries.php", line, "findOneBy")]);
}

// ------------------------------------------------- configuration and routes

const SERVICES_YAML: &str = "services:\n    _defaults:\n        autowire: true\n    mailer.default:\n        class: App\\Service\\Mailer\n    mailer: '@mailer.default'\n";
const ROUTES_YAML: &str = "home:\n    path: /\nabout: { path: /about }\n";
const CONTROLLER: &str = "src/Controller/HomeController.php";

/// The Symfony stubs with `config/services.yaml` and `config/routes.yaml`.
fn configured() -> Project {
    Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("config/services.yaml", SERVICES_YAML),
        ("config/routes.yaml", ROUTES_YAML),
    ])
}

/// A controller whose `index()` runs `body`.
fn controller(body: &str) -> String {
    format!(
        "<?php\nnamespace App\\Controller;\n\nuse Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController;\nuse Symfony\\Component\\Routing\\Generator\\UrlGeneratorInterface;\nuse Psr\\Container\\ContainerInterface;\n\nclass HomeController extends AbstractController\n{{\n    public function index(UrlGeneratorInterface $urls, ContainerInterface $c)\n    {{\n        {body}\n    }}\n}}\n"
    )
}

impl Project {
    fn complete_in_controller(&mut self, body: &str) -> Completions {
        let (text, pos) = caret(&controller(body));
        let doc = self.open(CONTROLLER, &text);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found
    }

    fn type_in_controller(&mut self, body: &str) -> String {
        let (text, pos) = caret(&controller(body));
        let doc = self.open(CONTROLLER, &text);
        let ty = self.engine.type_at(doc, pos).map(|ty| ty.to_string());
        self.engine.document_closed(doc);
        ty.unwrap_or_else(|| "<none>".to_owned())
    }
}

#[test]
fn route_names_complete_where_a_route_is_expected() {
    let mut project = configured();
    let found = project.complete_in_controller("return $this->redirectToRoute('/*|*/');");
    for label in ["home", "about", "blog_show"] {
        assert!(has(&found, label), "{label}");
    }
    let about = found.items.iter().find(|item| item.label == "about").unwrap();
    assert_eq!(about.insert_text, "about");
    assert_eq!(about.detail.as_deref(), Some("/about"));
    let show = found.items.iter().find(|item| item.label == "blog_show").unwrap();
    assert_eq!(show.detail.as_deref(), Some("/blog/{slug}"), "from the #[Route] attribute");

    let found = project.complete_in_controller("return $urls->generate('ab/*|*/');");
    assert!(has(&found, "about"));
    // Outside quotes the name is inserted quoted.
    let found = project.complete_in_controller("return path(/*|*/);");
    let home = found.items.iter().find(|item| item.label == "home").expect("home");
    assert_eq!(home.insert_text, "'home'");
    // Not where something else is expected.
    let found = project.complete_in_controller("return strlen('/*|*/');");
    assert!(!has(&found, "home"));
}

#[test]
fn undefined_route_names_are_reported() {
    let mut project = configured();
    let body = "$this->generateUrl('home');\n        $this->redirectToRoute('nope');\n        $urls->generate('blog_show');\n        path('missing');\n        $urls->generate($dynamic ?? 'x');";
    let doc = project.open(CONTROLLER, &controller(body));
    let inspected = project.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
    let routes: Vec<&str> = inspected
        .problems
        .iter()
        .filter(|problem| problem.inspection == "PhpUndefinedRoute")
        .map(|problem| problem.message.as_str())
        .collect();
    assert_eq!(routes, ["Route 'nope' is not defined", "Route 'missing' is not defined"]);
    let nope = inspected.problems.iter().find(|problem| problem.message.contains("nope")).unwrap();
    let text = controller(body);
    assert_eq!(&text[nope.start as usize..nope.end as usize], "nope");
    project.engine.document_closed(doc);

    // Without any route nothing is reported.
    let mut bare = Project::new(&[("lib/libraries.php", LIBRARIES), ("lib/bundle.php", FRAMEWORK_BUNDLE)]);
    let doc = bare.open(CONTROLLER, &controller("$this->redirectToRoute('nope');"));
    let inspected = bare.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
    assert!(!inspected.problems.iter().any(|problem| problem.inspection == "PhpUndefinedRoute"));
}

#[test]
fn configured_service_ids_are_typed_until_the_yaml_changes() {
    let mut project = configured();
    assert_eq!(project.type_in_controller("$c->get('mailer')/*|*/;"), "App\\Service\\Mailer");
    assert_eq!(project.type_in_controller("$c->get('mailer.default')/*|*/;"), "App\\Service\\Mailer");

    let yaml = project.root.join("config/services.yaml");
    std::fs::write(&yaml, "services:\n    mailer: { class: App\\Entity\\Product }\n").unwrap();
    project.engine.files_changed(std::slice::from_ref(&yaml), &[]);
    assert!(project.engine.wait_idle(WAIT));
    assert_eq!(project.type_in_controller("$c->get('mailer')/*|*/;"), "App\\Entity\\Product");
    assert_eq!(project.type_in_controller("$c->get('mailer.default')/*|*/;"), "mixed");
}

/// [`configured`] with another controller on disk that names routes.
fn with_usages() -> Project {
    let other = controller("$this->redirectToRoute('about');\n        $urls->generate('blog_show');")
        .replace("HomeController", "OtherController");
    Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("config/routes.yaml", ROUTES_YAML),
        ("src/Controller/OtherController.php", &other),
    ])
}

impl Project {
    /// The usages at the caret of `source` opened as `rel`, as `(relative
    /// path, line, the text at the range)`, the document left open.
    fn route_usages(&mut self, rel: &str, source: &str) -> Vec<(String, u32, String)> {
        let (text, pos) = caret(source);
        let doc = self.open(rel, &text);
        let usages = self.engine.references(doc, pos, true, &CancellationToken::new()).expect("handled").unwrap();
        let found = usages.iter().map(|usage| self.describe(&usage.path, usage.range)).collect();
        self.engine.document_closed(doc);
        found
    }

    /// The edits of renaming at the caret of `source` opened as `rel`, as
    /// `(relative path, the text replaced)`, sorted.
    fn route_rename(&mut self, rel: &str, source: &str, new_name: &str) -> Vec<(String, String)> {
        let (text, pos) = caret(source);
        let doc = self.open(rel, &text);
        let edit = self.engine.rename(doc, pos, new_name, &CancellationToken::new()).expect("handled").unwrap();
        let mut found = Vec::new();
        for file in &edit.files {
            let crate::base::FileEdit::Edit { path, edits, .. } = file else { panic!("only text edits") };
            for edit in edits {
                assert_eq!(edit.new_text, new_name);
                let (rel, _, replaced) = self.describe(path, edit.range);
                found.push((rel, replaced));
            }
        }
        self.engine.document_closed(doc);
        found.sort();
        found
    }
}

#[test]
fn a_route_name_goes_to_where_the_route_is_declared() {
    let mut project = configured();
    // A route of a YAML routes file: its key.
    let found = project.definition(CONTROLLER, &controller("return $this->redirectToRoute('ab/*|*/out');"));
    assert_eq!(found, Some(vec![target("config/routes.yaml", 2, "about")]));
    // A route of a `#[Route]` attribute: the name written there, after the class's prefix.
    let found = project.definition(CONTROLLER, &controller("return $urls->generate('blog_sh/*|*/ow');")).unwrap();
    let app = APP.lines().position(|line| line.contains("name: 'show'")).unwrap() as u32;
    assert_eq!(found, [target("lib/app.php", app, "show")]);
    // A name no route has goes nowhere; a string that names no route isn't one.
    assert_eq!(project.definition(CONTROLLER, &controller("return $urls->generate('no/*|*/pe');")), Some(vec![]));
    assert_eq!(project.definition(CONTROLLER, &controller("return strlen('ab/*|*/out');")), None);
}

#[test]
fn hover_on_a_route_name_shows_its_path() {
    let mut project = configured();
    let (text, pos) = caret(&controller("return $this->redirectToRoute('ab/*|*/out');"));
    let doc = project.open(CONTROLLER, &text);
    let hover = project.engine.hover(doc, pos).expect("a hover").markdown;
    assert!(hover.contains("**route** `about`"), "{hover}");
    assert!(hover.contains("`/about`") && hover.contains("config/routes.yaml"), "{hover}");
}

#[test]
fn usages_of_a_route_name_from_a_usage_and_from_its_attribute() {
    let mut project = with_usages();
    let found = project.route_usages(CONTROLLER, &controller("return $this->generateUrl('ab/*|*/out');"));
    assert_eq!(
        found,
        [
            target("config/routes.yaml", 2, "about"),
            target("src/Controller/HomeController.php", 11, "about"),
            target("src/Controller/OtherController.php", 11, "about"),
        ]
    );
    // From the attribute declaring it: the usages write the full name.
    let found = project.route_usages("lib/app.php", &APP.replace("name: 'show'", "name: 'sh/*|*/ow'"));
    let usages: Vec<&(String, u32, String)> = found.iter().filter(|(rel, ..)| rel.starts_with("src/")).collect();
    assert_eq!(usages, [&target("src/Controller/OtherController.php", 12, "blog_show")]);
}

#[test]
fn renaming_a_route_renames_its_declaration_and_its_usages() {
    let mut project = with_usages();
    let found = project.route_rename(CONTROLLER, &controller("return $this->generateUrl('ab/*|*/out');"), "about_us");
    assert_eq!(
        found,
        [
            ("config/routes.yaml".to_owned(), "about".to_owned()),
            ("src/Controller/HomeController.php".to_owned(), "about".to_owned()),
            ("src/Controller/OtherController.php".to_owned(), "about".to_owned()),
        ]
    );
    // The class's prefix stays: only the part the attribute writes changes.
    let found = project.route_rename(CONTROLLER, &controller("return $urls->generate('blog_sh/*|*/ow');"), "view");
    assert_eq!(
        found,
        [
            ("lib/app.php".to_owned(), "show".to_owned()),
            ("src/Controller/HomeController.php".to_owned(), "show".to_owned()),
            ("src/Controller/OtherController.php".to_owned(), "show".to_owned()),
        ]
    );
    // A name that can't be a route name is refused.
    let (text, pos) = caret(&controller("return $this->generateUrl('ab/*|*/out');"));
    let doc = project.open(CONTROLLER, &text);
    let refused = project.engine.rename(doc, pos, "not a name", &CancellationToken::new()).expect("handled");
    assert!(refused.is_err());
}

#[test]
fn route_names_are_off_where_the_call_takes_none() {
    let mut project = configured();
    // `generateUrl` of another class, `generate` of a controller.
    for body in ["(new \\App\\Service\\Mailer())->generateUrl('/*|*/');", "$this->generate('/*|*/');"] {
        let found = project.complete_in_controller(body);
        assert!(!has(&found, "home"), "{body}");
    }
}
