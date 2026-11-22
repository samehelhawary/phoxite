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

/// [`configured`] with Twig templates: a layout, a page extending it and
/// linking to routes, and a controller on disk rendering the page.
fn with_templates() -> Project {
    let other = controller("return $this->render('blog/show.html.twig', ['a' => 1]);")
        .replace("HomeController", "OtherController");
    Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("config/routes.yaml", ROUTES_YAML),
        ("src/Controller/OtherController.php", &other),
        ("templates/base.html.twig", "<a href=\"{{ path('about') }}\">About</a>\n{% block body %}{% endblock %}\n"),
        (
            "templates/blog/show.html.twig",
            "{% extends 'base.html.twig' %}\n{% block body %}<a href=\"{{ url('blog_show', { slug: 'a' }) }}\">x</a>{% endblock %}\n",
        ),
        ("templates/blog/list.html.twig", "{% include 'blog/show.html.twig' %}\n"),
    ])
}

/// Pairs of texts: what was replaced in which file, or a file's old and new place.
type Pairs = Vec<(String, String)>;

impl Project {
    /// What a rename at the caret of `source` opened as `rel` does: the
    /// texts replaced as `(relative path, the text)`, and the files moved as
    /// `(from, to)`, both sorted.
    fn renamed_with_files(&mut self, rel: &str, source: &str, new_name: &str) -> (Pairs, Pairs) {
        let (text, pos) = caret(source);
        let doc = self.open(rel, &text);
        let edit = self.engine.rename(doc, pos, new_name, &CancellationToken::new()).expect("handled").unwrap();
        let relative = |path: &Path| path.strip_prefix(&self.root).unwrap().to_string_lossy().replace('\\', "/");
        let (mut texts, mut moved) = (Vec::new(), Vec::new());
        for file in &edit.files {
            match file {
                crate::base::FileEdit::Edit { path, edits, .. } => {
                    for edit in edits {
                        assert_eq!(edit.new_text, new_name);
                        let (rel, _, replaced) = self.describe(path, edit.range);
                        texts.push((rel, replaced));
                    }
                }
                crate::base::FileEdit::Rename { from, to } => moved.push((relative(from), relative(to))),
                other => panic!("an edit or a move, not {other:?}"),
            }
        }
        self.engine.document_closed(doc);
        texts.sort();
        moved.sort();
        (texts, moved)
    }
}

#[test]
fn a_template_name_in_php_completes_and_goes_to_the_template() {
    let mut project = with_templates();
    let found = project.complete_in_controller("return $this->render('/*|*/');");
    for label in ["base.html.twig", "blog/show.html.twig", "blog/list.html.twig"] {
        assert!(has(&found, label), "{label}");
    }
    // Not in the second argument, and not where another string is expected.
    assert!(!has(&project.complete_in_controller("return $this->render('a.twig', ['/*|*/']);"), "base.html.twig"));
    assert!(!has(&project.complete_in_controller("return strlen('/*|*/');"), "base.html.twig"));

    let found = project.definition(CONTROLLER, &controller("return $this->renderView('blog/sh/*|*/ow.html.twig');"));
    assert_eq!(found, Some(vec![target("templates/blog/show.html.twig", 0, "")]));
    assert_eq!(
        project.definition(CONTROLLER, &controller("return $this->render('no/*|*/ne.html.twig');")),
        Some(vec![])
    );

    let (text, pos) = caret(&controller("return $this->render('blog/sh/*|*/ow.html.twig');"));
    let doc = project.open(CONTROLLER, &text);
    let hover = project.engine.hover(doc, pos).expect("a hover").markdown;
    assert!(hover.contains("**template** `blog/show.html.twig`") && hover.contains("templates/blog/show.html.twig"));
}

#[test]
fn usages_of_a_template_are_in_php_and_in_the_templates_naming_it() {
    let mut project = with_templates();
    let found = project.route_usages(CONTROLLER, &controller("return $this->render('blog/sh/*|*/ow.html.twig');"));
    assert_eq!(
        found,
        [
            // PHP: the open document and the controller on disk.
            target("src/Controller/HomeController.php", 11, "blog/show.html.twig"),
            target("src/Controller/OtherController.php", 11, "blog/show.html.twig"),
            // The template including it.
            target("templates/blog/list.html.twig", 0, "blog/show.html.twig"),
            // The template itself.
            target("templates/blog/show.html.twig", 0, ""),
        ]
    );
}

#[test]
fn renaming_a_template_moves_its_file_and_renames_it_everywhere() {
    let mut project = with_templates();
    let source = controller("return $this->render('blog/sh/*|*/ow.html.twig');");
    let (texts, moved) = project.renamed_with_files(CONTROLLER, &source, "blog/post.html.twig");
    assert_eq!(
        texts,
        [
            ("src/Controller/HomeController.php".to_owned(), "blog/show.html.twig".to_owned()),
            ("src/Controller/OtherController.php".to_owned(), "blog/show.html.twig".to_owned()),
            ("templates/blog/list.html.twig".to_owned(), "blog/show.html.twig".to_owned()),
        ]
    );
    assert_eq!(moved, [("templates/blog/show.html.twig".to_owned(), "templates/blog/post.html.twig".to_owned())]);

    // A name that is no template's, or would leave the template folders, is refused.
    let (text, pos) = caret(&source);
    let doc = project.open(CONTROLLER, &text);
    for bad in ["blog/post.html", "../outside.html.twig", "@Other/post.html.twig"] {
        let refused = project.engine.rename(doc, pos, bad, &CancellationToken::new()).expect("handled");
        assert!(refused.is_err(), "{bad}");
    }
    // A template that doesn't exist isn't renamed.
    let (text, pos) = caret(&controller("return $this->render('no/*|*/ne.html.twig');"));
    let doc = project.open("src/Controller/Missing.php", &text);
    assert!(project.engine.rename(doc, pos, "x.html.twig", &CancellationToken::new()).expect("handled").is_err());
}

#[test]
fn the_templates_naming_a_route_are_its_usages_and_are_renamed_with_it() {
    let mut project = with_templates();
    let found = project.route_usages(CONTROLLER, &controller("return $this->generateUrl('ab/*|*/out');"));
    assert_eq!(
        found,
        [
            target("config/routes.yaml", 2, "about"),
            target("src/Controller/HomeController.php", 11, "about"),
            target("templates/base.html.twig", 0, "about"),
        ]
    );
    // The class's prefix stays in the template too: only the part the attribute writes changes.
    let source = controller("return $urls->generate('blog_sh/*|*/ow');");
    let (texts, moved) = project.renamed_with_files(CONTROLLER, &source, "view");
    assert_eq!(
        texts,
        [
            ("lib/app.php".to_owned(), "show".to_owned()),
            ("src/Controller/HomeController.php".to_owned(), "show".to_owned()),
            ("templates/blog/show.html.twig".to_owned(), "show".to_owned()),
        ]
    );
    assert_eq!(moved, []);
}

#[test]
fn what_a_template_names_is_asked_by_the_kinds_id() {
    let project = with_templates();
    let cancel = CancellationToken::new();
    // Route names for completion in `path('…')`, with their paths.
    let routes = project.engine.symfony_name_items("symfonyRoute");
    assert!(routes.iter().any(|route| route.label == "about" && route.detail.as_deref() == Some("/about")));
    // Goto and usages from a template's side.
    let targets = project.engine.symfony_name_targets("symfonyRoute", "about").unwrap();
    assert_eq!(targets.len(), 1);
    let usages =
        project.engine.symfony_name_usages("symfonyTemplate", "base.html.twig", false, &cancel).unwrap().unwrap();
    assert_eq!(usages.len(), 1, "the page extending it");
    assert!(project.engine.symfony_name_renames("symfonyTemplate", "base.html.twig"));
    assert!(!project.engine.symfony_name_renames("symfonyTemplate", "@Bundle/base.html.twig"));

    // Outside a Symfony project nothing is known.
    let plain = Project::new(&[("templates/base.html.twig", "x")]);
    assert!(plain.engine.symfony_name_items("symfonyRoute").is_empty());
    assert!(plain.engine.symfony_name_targets("symfonyTemplate", "base.html.twig").is_none());
    assert!(plain.engine.symfony_name_usages("symfonyTemplate", "base.html.twig", false, &cancel).is_none());
    assert!(!plain.engine.symfony_name_renames("symfonyTemplate", "base.html.twig"));
}

/// The Symfony stubs with services and parameters in `config/services.yaml`.
fn with_container() -> Project {
    let yaml = "parameters:\n    app.locale: en\n    app.uploads: '%kernel.project_dir%/var/uploads'\nservices:\n    mailer.default:\n        class: App\\Service\\Mailer\n    mailer: '@mailer.default'\n";
    let other = controller("$c->get('mailer');\n        $this->getParameter('app.locale');")
        .replace("HomeController", "OtherController");
    Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("config/services.yaml", yaml),
        ("src/Controller/OtherController.php", &other),
    ])
}

#[test]
fn service_ids_complete_go_to_their_definition_and_have_usages() {
    let mut project = with_container();
    let found = project.complete_in_controller("$c->get('/*|*/');");
    let mailer = found.items.iter().find(|item| item.label == "mailer").expect("mailer");
    assert_eq!(mailer.detail.as_deref(), Some("App\\Service\\Mailer"), "the class its alias leads to");
    assert!(has(&found, "mailer.default"));
    assert!(!has(&found, "app.locale"), "a parameter is no service");

    let found = project.definition(CONTROLLER, &controller("$c->has('mai/*|*/ler');"));
    assert_eq!(found, Some(vec![target("config/services.yaml", 6, "mailer")]));
    // A class id needs no definition: nowhere to go, and nothing wrong.
    assert_eq!(project.definition(CONTROLLER, &controller("$c->get('App\\\\Ser/*|*/vice\\\\Mailer');")), Some(vec![]));

    let (text, pos) = caret(&controller("$c->get('mai/*|*/ler');"));
    let doc = project.open(CONTROLLER, &text);
    let hover = project.engine.hover(doc, pos).expect("a hover").markdown;
    assert!(hover.contains("**service** `mailer`") && hover.contains("App\\Service\\Mailer"), "{hover}");
    project.engine.document_closed(doc);

    let found = project.route_usages(CONTROLLER, &controller("$c->get('mai/*|*/ler');"));
    assert_eq!(
        found,
        [
            target("config/services.yaml", 6, "mailer"),
            target("src/Controller/HomeController.php", 11, "mailer"),
            target("src/Controller/OtherController.php", 11, "mailer"),
        ]
    );
}

/// A project whose YAML files and attributes name a service and a parameter too.
fn with_wiring() -> Project {
    let services = "parameters:\n    app.locale: en\n    app.greeting: 'Hello in %app.locale%'\nservices:\n    mailer.default:\n        class: App\\Service\\Mailer\n    mailer: '@mailer.default'\n    App\\Service\\Report:\n        arguments: ['@mailer.default', '@?mailer.default', '%app.locale%', '@@mailer.default', '@mailer.default.extra']\n";
    let attributes = "<?php\nnamespace Symfony\\Component\\DependencyInjection\\Attribute {\n    #[\\Attribute]\n    class Autowire { public function __construct($value = null, ?string $service = null, ?string $expression = null, ?string $env = null, ?string $param = null) {} }\n}\nnamespace Symfony\\Bridge\\Twig\\Attribute {\n    #[\\Attribute]\n    class Template { public function __construct(public string $template, public ?array $vars = null) {} }\n}\n";
    let wired = "<?php\nnamespace App\\Service;\n\nuse Symfony\\Bridge\\Twig\\Attribute\\Template;\nuse Symfony\\Component\\DependencyInjection\\Attribute\\Autowire;\n\nclass Wired\n{\n    public function __construct(\n        #[Autowire(service: 'mailer.default')] private object $mailer,\n        #[Autowire(param: 'app.locale')] private string $locale,\n    ) {}\n\n    #[Template('blog/show.html.twig')]\n    public function show(): array { return []; }\n}\n";
    Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("lib/attributes.php", attributes),
        ("config/services.yaml", services),
        ("config/packages/extra.yaml", "framework:\n    default_locale: '%app.locale%'\n"),
        ("src/Service/Wired.php", wired),
        ("templates/blog/show.html.twig", "x\n"),
    ])
}

#[test]
fn a_service_of_a_php_configuration_has_its_usages_there() {
    let services = "<?php\nreturn [\n    'parameters' => ['app.locale' => 'en', 'app.title' => 'Shop (%app.locale%)'],\n    'services' => [\n        'mailer.default' => ['class' => 'App\\Service\\Mailer'],\n        'mailer' => '@mailer.default',\n        'mailer.alias' => ['alias' => 'mailer.default'],\n        'mailer.default.extra' => ['class' => 'App\\Service\\Mailer'],\n    ],\n];\n";
    let mut project = Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("config/services.php", services),
    ]);
    let source = controller("$c->get('mailer.de/*|*/fault');");
    assert_eq!(project.definition(CONTROLLER, &source), Some(vec![target("config/services.php", 4, "mailer.default")]));
    let found = project.route_usages(CONTROLLER, &source);
    assert_eq!(
        found,
        [
            target("config/services.php", 4, "mailer.default"),
            target("config/services.php", 5, "mailer.default"),
            target("config/services.php", 6, "mailer.default"),
            target(CONTROLLER, 11, "mailer.default"),
        ]
    );
    let source = controller("$this->getParameter('app.lo/*|*/cale');");
    let found = project.route_usages(CONTROLLER, &source);
    assert_eq!(
        found,
        [
            target("config/services.php", 2, "app.locale"),
            target("config/services.php", 2, "app.locale"),
            target(CONTROLLER, 11, "app.locale"),
        ]
    );
}

#[test]
fn a_service_and_a_parameter_are_renamed_in_php_in_yaml_and_in_attributes() {
    let mut project = with_wiring();
    // The usages of a service: its key, `@id` and `@?id` in YAML, the attribute's argument, PHP.
    let found = project.route_usages(CONTROLLER, &controller("$c->get('mailer.de/*|*/fault');"));
    assert_eq!(
        found,
        [
            target("config/services.yaml", 4, "mailer.default"),
            target("config/services.yaml", 6, "mailer.default"),
            // `'@@mailer.default'` is an escaped `@`, and `mailer.default.extra` another service.
            target("config/services.yaml", 8, "mailer.default"),
            target("config/services.yaml", 8, "mailer.default"),
            target("src/Controller/HomeController.php", 11, "mailer.default"),
            target("src/Service/Wired.php", 9, "mailer.default"),
        ]
    );
    let source = controller("$c->get('mailer.de/*|*/fault');");
    let (texts, moved) = project.renamed_with_files(CONTROLLER, &source, "mailer.smtp");
    let files: Vec<&str> = texts.iter().map(|(file, _)| file.as_str()).collect();
    assert_eq!(
        files,
        [
            "config/services.yaml",
            "config/services.yaml",
            "config/services.yaml",
            "config/services.yaml",
            "src/Controller/HomeController.php",
            "src/Service/Wired.php"
        ]
    );
    assert!(texts.iter().all(|(_, replaced)| replaced == "mailer.default") && moved.is_empty());

    // A parameter, from the attribute naming it: every `%name%` below `config/` too.
    let wired = std::fs::read_to_string(project.root.join("src/Service/Wired.php")).unwrap();
    let source = wired.replace("param: 'app.locale'", "param: 'app.lo/*|*/cale'");
    let (texts, _) = project.renamed_with_files("src/Service/Wired.php", &source, "app.language");
    assert_eq!(
        texts,
        [
            ("config/packages/extra.yaml".to_owned(), "app.locale".to_owned()),
            ("config/services.yaml".to_owned(), "app.locale".to_owned()),
            ("config/services.yaml".to_owned(), "app.locale".to_owned()),
            ("config/services.yaml".to_owned(), "app.locale".to_owned()),
            ("src/Service/Wired.php".to_owned(), "app.locale".to_owned()),
        ]
    );

    // A service that is its class, one no file defines, a name that is no id: not renamed.
    for (body, name) in [("$c->get('App\\\\Service\\\\Re/*|*/port');", "x"), ("$c->get('unk/*|*/nown');", "x")] {
        let (text, pos) = caret(&controller(body));
        let doc = project.open(CONTROLLER, &text);
        assert!(project.engine.rename(doc, pos, name, &CancellationToken::new()).expect("handled").is_err(), "{body}");
        project.engine.document_closed(doc);
    }
    let (text, pos) = caret(&controller("$c->get('mailer.de/*|*/fault');"));
    let doc = project.open(CONTROLLER, &text);
    assert!(project.engine.rename(doc, pos, "App\\Mailer", &CancellationToken::new()).expect("handled").is_err());
}

#[test]
fn a_template_that_is_not_there_is_reported_when_that_is_certain() {
    let mut project = with_templates();
    let body = "$this->render('blog/show.html.twig');\n        $this->render('blog/missing.html.twig');\n        $this->render('@Bundle/layout.html.twig');\n        $this->render($name);\n        $this->render('notes.txt');";
    let doc = project.open(CONTROLLER, &controller(body));
    let inspected = project.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
    let found: Vec<&str> = inspected
        .problems
        .iter()
        .filter(|problem| problem.inspection == "PhpUndefinedTemplate")
        .map(|problem| problem.message.as_str())
        .collect();
    // Not a bundle's template, nor a name that is no Twig template's.
    assert_eq!(found, ["Template 'blog/missing.html.twig' not found"]);
    project.engine.document_closed(doc);

    // A project without any template may keep them where the engine doesn't look.
    let mut bare = configured();
    let doc = bare.open(CONTROLLER, &controller("$this->render('blog/missing.html.twig');"));
    let inspected = bare.engine.inspect(doc, &CancellationToken::new()).expect("inspected");
    assert!(!inspected.problems.iter().any(|problem| problem.inspection == "PhpUndefinedTemplate"));
}

#[test]
fn a_template_an_editor_has_open_is_read_as_the_editor_has_it() {
    let mut project = with_templates();
    let base = project.root.join("templates/base.html.twig");
    // Unsaved: the layout now links to `home` twice and no longer to `about`.
    project.engine.set_template_text(&base, Some("{{ path('home') }}\n{{ url('home') }}\n"));
    let found = project.route_usages(CONTROLLER, &controller("return $this->generateUrl('ho/*|*/me');"));
    let in_layout: Vec<u32> =
        found.iter().filter(|(file, ..)| file == "templates/base.html.twig").map(|(_, line, _)| *line).collect();
    assert_eq!(in_layout, [0, 1]);
    let found = project.route_usages(CONTROLLER, &controller("return $this->generateUrl('ab/*|*/out');"));
    assert!(!found.iter().any(|(file, ..)| file == "templates/base.html.twig"), "{found:?}");
    // Closed again: the file as it is saved.
    project.engine.set_template_text(&base, None);
    let found = project.route_usages(CONTROLLER, &controller("return $this->generateUrl('ab/*|*/out');"));
    assert!(found.iter().any(|(file, ..)| file == "templates/base.html.twig"), "{found:?}");
}

#[test]
fn a_member_of_a_templates_variable_goes_to_its_declaration() {
    use crate::frameworks::symfony::LoopVariable;

    let entities = "<?php\nnamespace App\\Blog;\n\nclass Post\n{\n    public string $title = '';\n    private string $secret = '';\n    public function getAuthor(): Author { return new Author(); }\n}\nclass Author { public string $name = ''; }\n";
    let shown = "<?php\nnamespace App\\Controller;\n\nuse App\\Blog\\Post;\nuse Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController;\n\nclass BlogPages extends AbstractController\n{\n    public function show(Post $post)\n    {\n        return $this->render('blog/show.html.twig', ['post' => $post, 'posts' => [$post]]);\n    }\n}\n";
    let project = Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("src/Blog/entities.php", entities),
        ("src/Controller/BlogPages.php", shown),
        ("templates/blog/show.html.twig", "{{ post.title }}\n"),
    ]);
    let member = |loops: &[LoopVariable], chain: &[&str], name: &str| {
        let chain: Vec<String> = chain.iter().map(|part| (*part).to_owned()).collect();
        project.engine.twig_member("blog/show.html.twig", loops, &chain, name).map(|(targets, ty)| {
            let at: Vec<(String, u32, String)> =
                targets.iter().map(|target| project.describe(&target.path, target.range)).collect();
            (at, ty)
        })
    };
    // A property; a getter under the name Twig reads it by; further along the chain.
    assert_eq!(
        member(&[], &["post"], "title"),
        Some((vec![target("src/Blog/entities.php", 5, "$title")], "string".into()))
    );
    let (at, ty) = member(&[], &["post"], "author").unwrap();
    assert_eq!(at, [target("src/Blog/entities.php", 7, "getAuthor")]);
    assert!(ty.contains("Author"), "{ty}");
    assert_eq!(member(&[], &["post", "author"], "name").unwrap().0, [target("src/Blog/entities.php", 9, "$name")]);
    // A loop's variable.
    let loops = [LoopVariable { name: "item".into(), source: vec!["posts".into()] }];
    assert_eq!(member(&loops, &["item"], "title").unwrap().0, [target("src/Blog/entities.php", 5, "$title")]);
    // What isn't public, isn't there, or is of something unknown.
    assert_eq!(member(&[], &["post"], "secret"), None);
    assert_eq!(member(&[], &["post"], "nothing"), None);
    assert_eq!(member(&[], &["unknown"], "title"), None);
}

#[test]
fn a_translation_key_is_renamed_in_every_catalogue_and_usage() {
    const LABELS: &str = "src/Service/Labels.php";
    let mut project = Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("lib/translator.php", TRANSLATOR),
        ("translations/messages.en.yaml", "welcome: Hello\nform:\n    title: Title\nmenu.home: Home\n"),
        ("translations/messages.fr.yaml", "welcome: Bonjour\nform:\n    title: Titre\nmenu:\n    home: Accueil\n"),
        ("translations/messages.de.json", "{\n  \"welcome\": \"Hallo\"\n}\n"),
        ("templates/home.html.twig", "<h1>{{ 'welcome'|trans }}</h1>{{ 'form.title'|trans }}\n"),
    ]);
    // A key every catalogue writes whole: all of it changes, in YAML, JSON, PHP and the template.
    let (texts, _) = project.renamed_with_files(LABELS, &labels("return $t->trans('wel/*|*/come');"), "greeting");
    assert_eq!(
        texts,
        [
            ("src/Service/Labels.php".to_owned(), "welcome".to_owned()),
            ("templates/home.html.twig".to_owned(), "welcome".to_owned()),
            ("translations/messages.de.json".to_owned(), "welcome".to_owned()),
            ("translations/messages.en.yaml".to_owned(), "welcome".to_owned()),
            ("translations/messages.fr.yaml".to_owned(), "welcome".to_owned()),
        ]
    );
    // A nested key: its last part, where it is written and in the usages.
    let (texts, _) = project.renamed_with_files(LABELS, &labels("return $t->trans('form.ti/*|*/tle');"), "heading");
    assert_eq!(
        texts,
        [
            ("src/Service/Labels.php".to_owned(), "title".to_owned()),
            ("templates/home.html.twig".to_owned(), "title".to_owned()),
            ("translations/messages.en.yaml".to_owned(), "title".to_owned()),
            ("translations/messages.fr.yaml".to_owned(), "title".to_owned()),
        ]
    );
    let refused = |project: &mut Project, body: &str, name: &str| {
        let (text, pos) = caret(&labels(body));
        let doc = project.open(LABELS, &text);
        let found = project.engine.rename(doc, pos, name, &CancellationToken::new()).expect("handled");
        project.engine.document_closed(doc);
        found.is_err()
    };
    // A part can't get a dot; a key one catalogue nests and another writes whole; a message that is no key.
    assert!(refused(&mut project, "return $t->trans('form.ti/*|*/tle');", "main.heading"));
    assert!(refused(&mut project, "return $t->trans('menu.ho/*|*/me');", "start"));
    assert!(refused(&mut project, "return $t->trans('Some te/*|*/xt');", "other"));
    // A key written whole may get dots.
    assert!(!refused(&mut project, "return $t->trans('wel/*|*/come');", "home.greeting"));
}

#[test]
fn a_service_after_alias_or_parent_is_a_usage_and_packages_define_parameters() {
    let services = "services:\n    mailer.default:\n        class: App\\Service\\Mailer\n    mailer:\n        alias: mailer.default\n    mailer.fast:\n        parent: 'mailer.default'\n    other:\n        alias: mailer.default.extra\n";
    let mut project = Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/app.php", APP),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("config/services.yaml", services),
        ("config/packages/app.yaml", "parameters:\n    app.per_page: 20\n"),
    ]);
    let found = project.route_usages(CONTROLLER, &controller("$c->get('mailer.de/*|*/fault');"));
    let yaml: Vec<u32> =
        found.iter().filter(|(file, ..)| file == "config/services.yaml").map(|(_, line, _)| *line).collect();
    assert_eq!(yaml, [1, 4, 6], "its key, after `alias:` and after `parent:`; not `mailer.default.extra`");

    // A parameter a file of `config/packages` defines, with no services file importing it.
    let found = project.complete_in_controller("$this->getParameter('/*|*/');");
    assert!(has(&found, "app.per_page"));
    let found = project.definition(CONTROLLER, &controller("$this->getParameter('app.per/*|*/_page');"));
    assert_eq!(found, Some(vec![target("config/packages/app.yaml", 1, "app.per_page")]));
}

#[test]
fn what_an_installed_package_adds_to_twig_is_known_too() {
    let installed = r#"{"packages": [{"name": "acme/twig-extra", "version": "1.0.0", "type": "library", "autoload": {"psr-4": {"Acme\\Twig\\": "src/"}}, "install-path": "../acme/twig-extra"}]}"#;
    let extension = "<?php\nnamespace Acme\\Twig;\n\nuse Twig\\TwigFilter;\n\nclass Extra\n{\n    public function getFilters(): array\n    {\n        return [new TwigFilter('shout', 'strtoupper')];\n    }\n}\n";
    let project = Project::new(&[
        ("composer.json", r#"{"require": {"acme/twig-extra": "^1.0"}}"#),
        ("vendor/composer/installed.json", installed),
        ("vendor/acme/twig-extra/src/Extra.php", extension),
        ("src/index.php", "<?php\n"),
    ]);
    let names = project.engine.twig_extension_names();
    let shout = names.iter().find(|name| name.name == "shout").expect("the package's filter");
    assert!(shout.file.as_ref().is_some_and(|file| file.ends_with("vendor/acme/twig-extra/src/Extra.php")));
}

#[test]
fn an_attributes_argument_names_a_template() {
    let mut project = with_wiring();
    let wired = std::fs::read_to_string(project.root.join("src/Service/Wired.php")).unwrap();
    let source = wired.replace("#[Template('blog/show.html.twig')]", "#[Template('blog/sh/*|*/ow.html.twig')]");
    let found = project.definition("src/Service/Wired.php", &source);
    assert_eq!(found, Some(vec![target("templates/blog/show.html.twig", 0, "")]));
    // And completes there.
    let typing = wired.replace("#[Template('blog/show.html.twig')]", "#[Template('/*|*/')]");
    let (text, pos) = caret(&typing);
    let doc = project.open("src/Service/Wired.php", &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    assert!(has(&found, "blog/show.html.twig"));
}

#[test]
fn parameters_complete_and_go_to_their_definition() {
    let mut project = with_container();
    let found = project.complete_in_controller("$this->getParameter('/*|*/');");
    let uploads = found.items.iter().find(|item| item.label == "app.uploads").expect("app.uploads");
    assert_eq!(uploads.detail.as_deref(), Some("%kernel.project_dir%/var/uploads"));
    assert!(has(&found, "app.locale") && !has(&found, "mailer"));

    let found = project.definition(CONTROLLER, &controller("$this->getParameter('app.lo/*|*/cale');"));
    assert_eq!(found, Some(vec![target("config/services.yaml", 1, "app.locale")]));
    // One the kernel defines: nowhere to go.
    assert_eq!(project.definition(CONTROLLER, &controller("$this->getParameter('kernel.de/*|*/bug');")), Some(vec![]));

    let found = project.route_usages(CONTROLLER, &controller("$this->getParameter('app.lo/*|*/cale');"));
    assert_eq!(
        found,
        [
            target("config/services.yaml", 1, "app.locale"),
            target("src/Controller/HomeController.php", 11, "app.locale"),
            target("src/Controller/OtherController.php", 12, "app.locale"),
        ]
    );
}

const TRANSLATOR: &str = "<?php\nnamespace Symfony\\Contracts\\Translation;\n\ninterface TranslatorInterface\n{\n    public function trans(string $id, array $parameters = []): string;\n}\n";

/// A service whose `label()` runs `body` with a translator `$t`.
fn labels(body: &str) -> String {
    format!(
        "<?php\nnamespace App\\Service;\n\nuse Symfony\\Contracts\\Translation\\TranslatorInterface;\n\nclass Labels\n{{\n    public function label(TranslatorInterface $t)\n    {{\n        {body}\n    }}\n}}\n"
    )
}

#[test]
fn translation_keys_complete_go_to_every_locale_and_count_the_templates() {
    const LABELS: &str = "src/Service/Labels.php";
    let mut project = Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("lib/translator.php", TRANSLATOR),
        ("translations/messages.en.yaml", "welcome: Hello\nform:\n    title: Title\n"),
        ("translations/messages.fr.yaml", "welcome: Bonjour\n"),
        ("templates/home.html.twig", "<h1>{{ 'welcome'|trans }}</h1>\n{{ 'welcome'|upper }}\n"),
    ]);
    let (text, pos) = caret(&labels("return $t->trans('/*|*/');"));
    let doc = project.open(LABELS, &text);
    let found = project.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
    project.engine.document_closed(doc);
    let welcome = found.items.iter().find(|item| item.label == "welcome").expect("welcome");
    assert!(welcome.detail.is_some(), "a translation as the detail");
    assert!(has(&found, "form.title"), "a nested key, joined by dots");

    // Goto: the key in each locale's catalogue.
    let found = project.definition(LABELS, &labels("return $t->trans('wel/*|*/come');")).unwrap();
    assert_eq!(
        found,
        [target("translations/messages.en.yaml", 0, "welcome"), target("translations/messages.fr.yaml", 0, "welcome")]
    );
    let found = project.definition(LABELS, &labels("return $t->trans('form.ti/*|*/tle');")).unwrap();
    assert_eq!(found, [target("translations/messages.en.yaml", 2, "title")]);
    // A message that is no key goes nowhere.
    assert_eq!(project.definition(LABELS, &labels("return $t->trans('Some te/*|*/xt');")), Some(vec![]));

    let (text, pos) = caret(&labels("return $t->trans('wel/*|*/come');"));
    let doc = project.open(LABELS, &text);
    let hover = project.engine.hover(doc, pos).expect("a hover").markdown;
    assert!(hover.contains("`en` (messages): Hello") && hover.contains("`fr` (messages): Bonjour"), "{hover}");
    project.engine.document_closed(doc);

    // Usages: PHP, and the template's `|trans` (not its `|upper`).
    let found = project.route_usages(LABELS, &labels("return $t->trans('wel/*|*/come');"));
    assert_eq!(
        found,
        [
            target("src/Service/Labels.php", 9, "welcome"),
            target("templates/home.html.twig", 0, "welcome"),
            target("translations/messages.en.yaml", 0, "welcome"),
            target("translations/messages.fr.yaml", 0, "welcome"),
        ]
    );
    // And from a template's side.
    let targets = project.engine.symfony_name_targets("symfonyTranslation", "welcome").unwrap();
    assert_eq!(targets.len(), 2);
}

const BLOG: &str = "<?php\nnamespace Doctrine\\Common\\Collections {\n    interface Collection {}\n}\nnamespace App\\Blog {\n    use Doctrine\\Common\\Collections\\Collection;\n    use Doctrine\\ORM\\Mapping as ORM;\n\n    class Post\n    {\n        public string $title = '';\n        #[ORM\\OneToMany(targetEntity: Comment::class, mappedBy: 'post')]\n        public Collection $comments;\n        #[ORM\\ManyToOne(targetEntity: Author::class)]\n        public $author;\n        /** @var Collection<int, Author> */\n        #[ORM\\ManyToMany(targetEntity: Comment::class)]\n        public Collection $documented;\n        public Collection $plain;\n    }\n    class Comment { public string $body = ''; }\n    class Author { public string $name = ''; }\n}\n";

#[test]
fn doctrine_associations_type_their_properties() {
    let mut project = Project::new(&[("lib/libraries.php", LIBRARIES), ("lib/blog.php", BLOG)]);
    let mut type_of = |body: &str| {
        let (text, pos) = caret(&format!("<?php\nnamespace Main;\nuse App\\Blog\\Post;\n{body}\n"));
        let doc = project.open(MAIN, &text);
        let ty = project.engine.type_at(doc, pos).map(|ty| ty.to_string()).unwrap_or_default();
        project.engine.document_closed(doc);
        ty
    };
    // To many: a collection of the target.
    assert_eq!(
        type_of("function f(Post $p) { $p->comments/*|*/; }"),
        "Doctrine\\Common\\Collections\\Collection<int, App\\Blog\\Comment>"
    );
    // To one, without a type of its own: the target.
    let author = type_of("function f(Post $p) { $p->author/*|*/; }");
    assert!(author.contains("App\\Blog\\Author"), "{author}");
    // What the docblock says wins, and a collection that is no association stays one.
    let documented = type_of("function f(Post $p) { $p->documented/*|*/; }");
    assert!(documented.contains("Author") && !documented.contains("Comment"), "{documented}");
    assert_eq!(type_of("function f(Post $p) { $p->plain/*|*/; }"), "Doctrine\\Common\\Collections\\Collection");
}

#[test]
fn the_fields_of_an_entity_complete_in_a_repositorys_criteria() {
    let mut project = symfony(true);
    let found = project.complete_symfony("function f(ProductRepository $r) { $r->findBy(['/*|*/' => 1]); }");
    assert!(has(&found, "id") && has(&found, "name"), "{:?}", found.items.iter().map(|i| &i.label).collect::<Vec<_>>());
    assert!(!has(&found, "instances"), "a static property is no field");
    // The order's keys too; a value isn't a field, nor are other methods' arrays.
    let found = project.complete_symfony("function f(ProductRepository $r) { $r->findBy([], ['/*|*/' => 'ASC']); }");
    assert!(has(&found, "name"));
    let found = project.complete_symfony("function f(ProductRepository $r) { $r->findBy(['name' => '/*|*/']); }");
    assert!(!has(&found, "name"));

    // Goto: the property of the entity.
    let source = main("function f(ProductRepository $r) { $r->findOneBy(['na/*|*/me' => 'a']); }");
    let found = project.definition(MAIN, &source).unwrap();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!((found[0].0.as_str(), found[0].2.as_str()), ("lib/app.php", "$name"));
    let hover = project.hover_symfony("function f(ProductRepository $r) { $r->count(['na/*|*/me' => 'a']); }");
    assert!(hover.contains("**field** `name` of `App\\Entity\\Product`") && hover.contains("string"), "{hover}");
}

#[test]
fn what_php_adds_to_twig_is_known_with_where_it_is_written() {
    use crate::summary::TwigNameKind;
    let extension = "<?php\nnamespace App\\Twig;\n\nuse Twig\\Attribute\\AsTwigFunction;\nuse Twig\\TwigFilter;\nuse Twig\\TwigTest;\n\nclass AppExtension\n{\n    public function getFilters(): array\n    {\n        return [new TwigFilter('price', [$this, 'price'])];\n    }\n\n    public function getTests(): array\n    {\n        return [new TwigTest('expensive', fn ($x) => $x > 10)];\n    }\n\n    #[AsTwigFunction('money')]\n    public function money(): string\n    {\n        return '';\n    }\n}\n";
    let project = Project::new(&[("src/Twig/AppExtension.php", extension)]);
    let symfony = project.engine.resolver().symfony();
    let mut found: Vec<(TwigNameKind, &str, u32)> =
        symfony.twig_names.iter().map(|name| (name.kind, name.name.as_str(), name.range.start.line)).collect();
    // The attributes of a class are read before what its methods instantiate.
    found.sort_by_key(|(_, _, line)| *line);
    assert_eq!(
        found,
        [
            (TwigNameKind::Filter, "price", 11),
            (TwigNameKind::Test, "expensive", 16),
            (TwigNameKind::Function, "money", 19)
        ]
    );
    assert!(symfony.twig_names[0].file.as_ref().is_some_and(|file| file.ends_with("AppExtension.php")));
}

#[test]
fn a_template_knows_what_php_passes_it_and_what_follows_a_dot() {
    use crate::frameworks::symfony::LoopVariable;

    let entities = "<?php\nnamespace App\\Blog;\n\nclass Post\n{\n    public string $title = '';\n    private string $secret = '';\n    /** @var list<Comment> */\n    public array $comments = [];\n    public function getAuthor(): Author { return new Author(); }\n    public function isPublished(): bool { return true; }\n    public function slug(string $prefix): string { return ''; }\n    protected function hidden(): int { return 0; }\n}\nclass Comment { public string $body = ''; public function getAuthor(): Author { return new Author(); } }\nclass Author { public string $name = ''; }\n";
    let shown = "<?php\nnamespace App\\Controller;\n\nuse App\\Blog\\Post;\nuse Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController;\n\nclass BlogPages extends AbstractController\n{\n    public function show(Post $post)\n    {\n        return $this->render('blog/show.html.twig', ['post' => $post, 'count' => 3]);\n    }\n\n    public function preview(Post $draft)\n    {\n        return $this->render('blog/show.html.twig', ['post' => $draft, 'preview' => true]);\n    }\n}\n";
    let project = Project::new(&[
        ("lib/libraries.php", LIBRARIES),
        ("lib/bundle.php", FRAMEWORK_BUNDLE),
        ("src/Blog/entities.php", entities),
        ("src/Controller/BlogPages.php", shown),
        ("templates/blog/show.html.twig", "{{ post.title }}\n"),
    ]);

    // The variables of every place rendering the template.
    let variables = project.engine.twig_variables("blog/show.html.twig");
    let named: Vec<(&str, &str)> = variables.iter().map(|(name, ty)| (name.as_str(), ty.as_str())).collect();
    assert_eq!(named, [("count", "int"), ("post", "App\\Blog\\Post"), ("preview", "bool")]);
    assert!(project.engine.twig_variables("other.html.twig").is_empty());

    let members = |loops: &[LoopVariable], chain: &[&str]| -> Vec<(String, String)> {
        let chain: Vec<String> = chain.iter().map(|name| (*name).to_owned()).collect();
        let found = project.engine.twig_members("blog/show.html.twig", loops, &chain);
        found.into_iter().map(|member| (member.name, member.detail)).collect()
    };
    let has = |found: &[(String, String)], name: &str, ty: &str| found.iter().any(|(n, t)| n == name && t.contains(ty));
    // After `post.`: public properties, getters under the name Twig reads them by, other methods.
    let post = members(&[], &["post"]);
    assert!(has(&post, "title", "string") && has(&post, "author", "Author") && has(&post, "published", "bool"));
    assert!(has(&post, "slug", "string") && has(&post, "comments", "Comment"), "{post:?}");
    assert!(!post.iter().any(|(name, _)| name == "secret" || name == "hidden" || name == "getAuthor"), "{post:?}");
    // Further along the chain, through a getter.
    assert_eq!(members(&[], &["post", "author"]), [("name".to_owned(), "string".to_owned())]);
    // A loop's variable is an element of what the loop goes through.
    let loops = [LoopVariable { name: "comment".into(), source: vec!["post".into(), "comments".into()] }];
    let comment = members(&loops, &["comment"]);
    assert!(has(&comment, "body", "string") && has(&comment, "author", "Author"), "{comment:?}");
    assert_eq!(members(&loops, &["comment", "author"]), [("name".to_owned(), "string".to_owned())]);
    // What isn't known has no members.
    assert!(members(&[], &["nothing"]).is_empty() && members(&[], &["count"]).is_empty());
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
