use std::path::PathBuf;

use crate::base::{Pos, SymbolKind};

use super::*;
use crate::syntax::LineIndex;

const CARET: &str = "/*|*/";

/// The text without its caret marker, and the caret's position.
fn caret(src: &str) -> (String, Pos) {
    let at = src.find(CARET).expect("caret marker");
    let text = format!("{}{}", &src[..at], &src[at + CARET.len()..]);
    let pos = LineIndex::new(&text).pos(at as u32);
    (text, pos)
}

fn labels(src: &str) -> Vec<String> {
    let (text, pos) = caret(src);
    completions(&text, pos, false, &TemplatePaths::default()).into_iter().map(|c| c.label).collect()
}

fn kinds(src: &str) -> Vec<(String, TwigCompletionKind)> {
    let (text, pos) = caret(src);
    completions(&text, pos, false, &TemplatePaths::default()).into_iter().map(|c| (c.label, c.kind)).collect()
}

fn has(labels: &[String], label: &str) -> bool {
    labels.iter().any(|l| l == label)
}

// --- Outline -----------------------------------------------------------------

#[test]
fn outline_lists_parent_blocks_macros_and_sets() {
    let text = "{% extends 'base.html.twig' %}\n\
                {% import 'forms.html.twig' as forms %}\n\
                {% from 'macros.twig' import input as field, label %}\n\
                {% set title = 'Home' %}\n\
                {% block body %}\n  {% block content %}x{% endblock %}\n  {% if a %}{% block side %}{% endblock %}{% endif %}\n{% endblock %}\n\
                {% block head 'short' %}\n\
                {% macro button(label, type = 'submit') %}<b>{{ label }}</b>{% endmacro %}\n";
    let items = outline(text);
    let summary: Vec<(String, SymbolKind, Option<String>)> =
        items.iter().map(|i| (i.name.clone(), i.kind, i.detail.clone())).collect();
    assert_eq!(
        summary,
        vec![
            ("base.html.twig".into(), SymbolKind::File, Some("extends".into())),
            ("forms".into(), SymbolKind::Namespace, Some("forms.html.twig".into())),
            ("field".into(), SymbolKind::Function, Some("macros.twig".into())),
            ("label".into(), SymbolKind::Function, Some("macros.twig".into())),
            ("title".into(), SymbolKind::Variable, None),
            ("body".into(), SymbolKind::Module, None),
            ("head".into(), SymbolKind::Module, None),
            ("button".into(), SymbolKind::Function, Some("(label, type)".into())),
        ]
    );
    let body = &items[5];
    let children: Vec<&str> = body.children.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(children, ["content", "side"]);
    // The selection is the block's name.
    assert_eq!(body.selection_range.start, Pos { line: 4, col: 9 });
    assert_eq!(body.range.start, Pos { line: 4, col: 0 });
    assert_eq!(body.range.end.line, 7);
}

#[test]
fn outline_of_an_embed_holds_its_blocks() {
    let items = outline("{% embed 'card.twig' with {a: 1} %}{% block title %}T{% endblock %}{% endembed %}");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "card.twig");
    assert_eq!(items[0].detail.as_deref(), Some("embed"));
    assert_eq!(items[0].children[0].name, "title");
}

#[test]
fn outline_survives_broken_templates() {
    let items = outline("{% block a %}ok{% endblock %}\n{% if %}\n{{ 'é' ");
    assert_eq!(items.first().map(|i| i.name.as_str()), Some("a"));
    assert!(outline("").is_empty());
}

// --- Template references and goto ---------------------------------------------

#[test]
fn template_refs_cover_every_loading_construct() {
    let text = "{% extends ['a.twig', 'b.twig'] %}{% use 'blocks.twig' %}\n\
                {% include 'inc.twig' with {x: 'not-a-template'} %}{% embed \"emb.twig\" %}{% endembed %}\n\
                {% import 'm.twig' as m %}{% from 'f.twig' import x %}\n\
                {{ include('fn.twig', {y: 'nope'}) }}{{ source(name: 'src.twig') }}{{ include(cond ? 'c1.twig' : 'c2.twig') }}\n\
                {{ other('no.twig') }}";
    let names: Vec<String> = template_refs(text).into_iter().map(|r| r.name).collect();
    assert_eq!(
        names,
        [
            "a.twig",
            "b.twig",
            "blocks.twig",
            "inc.twig",
            "emb.twig",
            "m.twig",
            "f.twig",
            "fn.twig",
            "src.twig",
            "c1.twig",
            "c2.twig"
        ]
    );
}

#[test]
fn template_ref_range_is_the_string_content() {
    let (text, pos) = caret("{% extends 'layout/ba/*|*/se.html.twig' %}");
    let found = template_at(&text, pos).unwrap();
    assert_eq!(found.name, "layout/base.html.twig");
    assert_eq!(found.range.start, Pos { line: 0, col: 12 });
    assert_eq!(found.range.end, Pos { line: 0, col: 33 });
    let (text, pos) = caret("{% extends 'base.twig' %}{{ ti/*|*/tle }}");
    assert_eq!(template_at(&text, pos), None);
}

fn fixture_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for file in [
        "templates/base.html.twig",
        "templates/admin/list.html.twig",
        "templates/admin/notes.txt",
        "lib/emails/welcome.twig",
        "resources/views/legacy.twig",
    ] {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{{ x }}").unwrap();
    }
    std::fs::create_dir_all(root.join("config/packages")).unwrap();
    std::fs::write(
        root.join("config/packages/twig.yaml"),
        "twig:\n    default_path: '%kernel.project_dir%/templates'\n    paths:\n        'lib/emails': Email\n",
    )
    .unwrap();
    dir
}

#[test]
fn template_paths_resolve_plain_and_namespaced_names() {
    let dir = fixture_project();
    let root = dir.path();
    let paths = TemplatePaths::discover(root);
    assert_eq!(paths.resolve("base.html.twig"), Some(root.join("templates").join("base.html.twig")));
    assert_eq!(paths.resolve("admin/list.html.twig"), Some(root.join("templates/admin").join("list.html.twig")));
    assert_eq!(paths.resolve("@Email/welcome.twig"), Some(root.join("lib/emails").join("welcome.twig")));
    assert_eq!(paths.resolve("legacy.twig"), Some(root.join("resources/views").join("legacy.twig")));
    assert_eq!(paths.resolve("welcome.twig"), None);
    assert_eq!(paths.resolve("../config/packages/twig.yaml"), None);
    assert_eq!(paths.resolve("missing.twig"), None);
    assert_eq!(paths.names(), ["@Email/welcome.twig", "admin/list.html.twig", "base.html.twig", "legacy.twig"]);
}

#[test]
fn goto_template_opens_the_file() {
    let dir = fixture_project();
    let paths = TemplatePaths::discover(dir.path());
    let (text, pos) = caret("{% extends 'base/*|*/.html.twig' %}");
    let (file, origin) = goto_template(&text, pos, &paths).unwrap();
    assert_eq!(file, dir.path().join("templates").join("base.html.twig"));
    assert_eq!(origin.start.col, 12);
    let (text, pos) = caret("{{ include('@Email/wel/*|*/come.twig') }}");
    assert_eq!(goto_template(&text, pos, &paths).unwrap().0, dir.path().join("lib/emails").join("welcome.twig"));
    let (text, pos) = caret("{% include 'nowhere/*|*/.twig' %}");
    assert_eq!(goto_template(&text, pos, &paths), None);
}

// --- Completion ---------------------------------------------------------------

#[test]
fn completes_tags_at_the_start_of_a_statement() {
    let found = kinds("{% fo/*|*/ %}");
    assert!(found.contains(&("for".into(), TwigCompletionKind::Tag)));
    assert!(found.contains(&("extends".into(), TwigCompletionKind::Tag)));
    assert!(!found.iter().any(|(_, kind)| *kind != TwigCompletionKind::Tag));
    assert!(has(&labels("{%- /*|*/"), "block"));
    // Not a tag position: the expression after the tag name.
    assert!(!has(&labels("{% if fo/*|*/ %}"), "for"));
}

#[test]
fn completes_filters_after_a_pipe_and_in_apply() {
    let found = kinds("{% set name = 'x' %}{{ name|up/*|*/ }}");
    assert!(found.contains(&("upper".into(), TwigCompletionKind::Filter)));
    assert!(found.iter().all(|(_, kind)| *kind == TwigCompletionKind::Filter));
    assert!(has(&labels("{{ name | /*|*/"), "date"));
    assert!(has(&labels("{{ items|map(i => i)|/*|*/ }}"), "join"));
    assert!(has(&labels("{% apply /*|*/ %}x{% endapply %}"), "lower"));
    assert!(has(&labels("{% apply lower|/*|*/ %}x{% endapply %}"), "escape"));
}

#[test]
fn completes_tests_after_is() {
    assert!(has(&labels("{% if x is /*|*/ %}{% endif %}"), "defined"));
    assert!(has(&labels("{% if x is not em/*|*/ %}{% endif %}"), "empty"));
    assert!(has(&labels("{{ (x is /*|*/) }}"), "divisible by"));
    assert!(!has(&labels("{% if this /*|*/ %}{% endif %}"), "defined"));
}

#[test]
fn completes_variables_in_scope() {
    let src = "{% import 'forms.twig' as forms %}{% from 'm.twig' import field %}\n\
               {% set title = 'Home' %}\n\
               {% for item in items %}\n  {{ /*|*/ }}\n{% endfor %}\n\
               {% macro row(cells, sep = ',') %}{% set inner = 1 %}{% endmacro %}";
    let found = kinds(src);
    for (label, kind) in [
        ("title", TwigCompletionKind::Variable),
        ("item", TwigCompletionKind::Variable),
        ("loop", TwigCompletionKind::Variable),
        ("forms", TwigCompletionKind::Variable),
        ("field", TwigCompletionKind::Macro),
        ("row", TwigCompletionKind::Macro),
        ("_self", TwigCompletionKind::Variable),
        ("range", TwigCompletionKind::Function),
    ] {
        assert!(found.contains(&(label.into(), kind)), "{label} missing from {found:?}");
    }
    // Macro arguments and sets stay inside the macro.
    let labels: Vec<String> = found.into_iter().map(|(l, _)| l).collect();
    for absent in ["cells", "sep", "inner", "path"] {
        assert!(!has(&labels, absent), "{absent} shouldn't be in scope");
    }
}

#[test]
fn loop_variables_stay_in_the_loop() {
    let src = "{% for user in users %}{{ user }}{% else %}{{ /*|*/ }}{% endfor %}";
    let found = labels(src);
    assert!(!has(&found, "user") && !has(&found, "loop"));
    let src = "{% for k, v in map %}{% endfor %}{{ /*|*/ }}";
    let found = labels(src);
    assert!(!has(&found, "k") && !has(&found, "v"));
    let src = "{% for k, v in map %}{% if k %}{{ v.na }}{{ /*|*/ }}{% endif %}{% endfor %}";
    let found = labels(src);
    assert!(has(&found, "k") && has(&found, "v") && has(&found, "loop"));
    let src = "{% macro m(a, b) %}{{ /*|*/ }}{% endmacro %}";
    let found = labels(src);
    assert!(has(&found, "a") && has(&found, "b"));
}

#[test]
fn half_written_tags_keep_the_scope() {
    // The tag being typed doesn't parse; the loop around it still counts.
    let found = labels("{% for row in rows %}\n{{ row| }}{% if ro/*|*/\n{% endfor %}");
    assert!(has(&found, "row"), "{found:?}");
    let found = labels("{% set total = 3 %}{% for row in rows %}{{ to/*|*/ {% endfor %}");
    assert!(has(&found, "total") && has(&found, "row"), "{found:?}");
}

#[test]
fn completion_replaces_the_word_at_the_caret() {
    let (text, pos) = caret("{{ name|lo/*|*/wer }}");
    let item =
        completions(&text, pos, false, &TemplatePaths::default()).into_iter().find(|c| c.label == "lower").unwrap();
    assert_eq!(item.replace.start.col, 8);
    assert_eq!(item.replace.end.col, 13);
    assert_eq!(item.documentation.as_deref(), Some("Lowercases a string."));
    assert_eq!(item.detail, None);
    let (text, pos) = caret("{{ x|date/*|*/ }}");
    let item =
        completions(&text, pos, false, &TemplatePaths::default()).into_iter().find(|c| c.label == "date").unwrap();
    assert_eq!(item.detail.as_deref(), Some("(format = null, timezone = null)"));
}

#[test]
fn nothing_outside_tags_in_comments_or_after_a_dot() {
    assert!(labels("<p>fo/*|*/</p>").is_empty());
    assert!(labels("{{ x }} te/*|*/xt").is_empty());
    assert!(labels("{# a comment /*|*/ #}").is_empty());
    assert!(labels("{{ user./*|*/ }}").is_empty());
    assert!(labels("{{ 'just a str/*|*/ing' }}").is_empty());
    // Right after the delimiter, before a whitespace modifier.
    assert!(has(&labels("{{/*|*/- x }}"), "range"));
    assert!(has(&labels("{%/*|*/- if x %}{% endif %}"), "if"));
}

#[test]
fn symfony_adds_its_filters_functions_and_globals() {
    let (text, pos) = caret("{{ /*|*/ }}");
    let plain: Vec<String> =
        completions(&text, pos, false, &TemplatePaths::default()).into_iter().map(|c| c.label).collect();
    assert!(!has(&plain, "path") && !has(&plain, "app"));
    let symfony: Vec<TwigCompletion> = completions(&text, pos, true, &TemplatePaths::default());
    let path = symfony.iter().find(|c| c.label == "path").unwrap();
    assert_eq!(path.kind, TwigCompletionKind::Function);
    assert!(symfony.iter().any(|c| c.label == "app"));
    let (text, pos) = caret("{{ 'key'|/*|*/ }}");
    assert!(completions(&text, pos, true, &TemplatePaths::default()).iter().any(|c| c.label == "trans"));
}

#[test]
fn completes_template_names_in_template_strings() {
    let dir = fixture_project();
    let paths = TemplatePaths::discover(dir.path());
    let complete = |src: &str| {
        let (text, pos) = caret(src);
        completions(&text, pos, false, &paths)
    };
    let found = complete("{% extends 'ba/*|*/' %}");
    let names: Vec<&str> = found.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(names, ["@Email/welcome.twig", "admin/list.html.twig", "base.html.twig", "legacy.twig"]);
    assert!(found.iter().all(|c| c.kind == TwigCompletionKind::Template));
    // The whole name up to the closing quote is replaced.
    assert_eq!(found[0].replace.start.col, 12);
    assert_eq!(found[0].replace.end.col, 14);
    assert_eq!(complete("{{ include(\"/*|*/\") }}").len(), 4);
    assert_eq!(complete("{% include 'a' with {k: '/*|*/'} %}").len(), 0);
    assert_eq!(complete("{{ dump('/*|*/') }}").len(), 0);
}

#[test]
fn template_paths_without_config() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(TemplatePaths::discover(dir.path()), TemplatePaths::default());
    let paths = TemplatePaths::from_dirs(vec![(None, PathBuf::from("/nowhere"))]);
    assert!(paths.names().is_empty());
}

// --- Problems ----------------------------------------------------------------

/// The problems of `text` as `(the text at the range, message)`.
fn found_problems(text: &str) -> Vec<(String, String)> {
    let index = LineIndex::new(text);
    problems(text)
        .into_iter()
        .map(|problem| {
            assert_eq!(problem.source.as_deref(), Some(DIAGNOSTIC_SOURCE));
            assert_eq!(problem.code.as_deref(), Some(SYNTAX_CODE));
            let (start, end) = index.byte_range(problem.range);
            (text[start as usize..end as usize].to_owned(), problem.message)
        })
        .collect()
}

#[test]
fn a_valid_template_has_no_problem() {
    let text = "{% extends 'base.html.twig' %}\n{% block body %}\n  {% for item in items %}{{ item.name|upper }}{% else %}none{% endfor %}\n  {% if a is defined and a > 1 %}{{ a }}{% endif %}\n{% endblock %}\n{# a comment #}\n";
    assert_eq!(found_problems(text), []);
    assert_eq!(found_problems(""), []);
    assert_eq!(found_problems("plain text, no tag"), []);
}

#[test]
fn a_block_left_open_is_reported_at_its_tag() {
    let found = found_problems("{% if a %}\n  {% for x in y %}{{ x }}{% endfor %}\n");
    assert_eq!(found, [("{% if a %}".to_owned(), "Unclosed 'if' block".to_owned())]);
    let found = found_problems("{% block body %}{% if a %}x{% endblock %}");
    assert_eq!(found.iter().map(|(at, _)| at.as_str()).collect::<Vec<_>>(), ["{% if a %}"]);
}

#[test]
fn every_broken_tag_is_reported_and_the_rest_is_still_read() {
    let text = "{{ a| }}\n{% set b = 1 %}\n{{ b + }}\n{{ fine }}\n";
    let found = found_problems(text);
    let at: Vec<&str> = found.iter().map(|(at, _)| at.as_str()).collect();
    assert_eq!(at.len(), 2, "{found:?}");
    assert!(found.iter().all(|(_, message)| !message.is_empty()));
    // Each one is inside its own tag.
    let lines: Vec<u32> = problems(text).iter().map(|problem| problem.range.start.line).collect();
    assert_eq!(lines, [0, 2]);
}

#[test]
fn a_tag_an_extension_adds_is_no_problem() {
    assert_eq!(found_problems("{% trans %}Hello{% endtrans %}\n{% form_theme form 'x.html.twig' %}\n"), []);
    // The error after it is still found.
    let found = found_problems("{% stopwatch 'x' %}{{ a| }}{% endstopwatch %}");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("filter"), "{found:?}");
}

// --- Hover -------------------------------------------------------------------

fn hover_at(src: &str, symfony: bool) -> Option<(String, String)> {
    let (text, pos) = caret(src);
    let (markdown, range) = hover(&text, pos, symfony)?;
    let (start, end) = LineIndex::new(&text).byte_range(range);
    Some((text[start as usize..end as usize].to_owned(), markdown))
}

#[test]
fn hover_says_what_a_builtin_does() {
    let (word, markdown) = hover_at("{{ name|up/*|*/per }}", false).unwrap();
    assert_eq!(word, "upper");
    assert!(markdown.contains("upper") && markdown.contains("(filter)"), "{markdown}");
    let (word, markdown) = hover_at("{% ext/*|*/ends 'a.twig' %}", false).unwrap();
    assert_eq!(word, "extends");
    assert!(markdown.contains("(tag)"), "{markdown}");
    let (_, markdown) = hover_at("{{ ra/*|*/nge(1, 3) }}", false).unwrap();
    assert!(markdown.contains("range(") && markdown.contains("(function)"), "{markdown}");
    let (_, markdown) = hover_at("{% if a is def/*|*/ined %}{% endif %}", false).unwrap();
    assert!(markdown.contains("(test)"), "{markdown}");
    // Outside a tag, on a variable of the template, in a string: nothing.
    assert_eq!(hover_at("up/*|*/per {{ a }}", false), None);
    assert_eq!(hover_at("{% set title = 1 %}{{ ti/*|*/tle }}", false), None);
}

#[test]
fn hover_knows_symfony_only_with_its_bridge() {
    assert_eq!(hover_at("{{ pa/*|*/th('home') }}", false), None);
    let (_, markdown) = hover_at("{{ pa/*|*/th('home') }}", true).unwrap();
    assert!(markdown.contains("path(route_name"), "{markdown}");
}

#[test]
fn hover_shows_the_arguments_of_a_macro() {
    let (word, markdown) =
        hover_at("{% macro button(label, type = 'submit') %}{% endmacro %}{{ _self.but/*|*/ton('x') }}", false)
            .unwrap();
    assert_eq!(word, "button");
    assert!(markdown.contains("macro button(label, type)"), "{markdown}");
    let (_, markdown) = hover_at("{% from 'forms.twig' import input %}{{ in/*|*/put('x') }}", false).unwrap();
    assert!(markdown.contains("forms.twig"), "{markdown}");
}

// --- Goto and usages -----------------------------------------------------------

/// A project with a layout, a template of macros and a page using both.
fn linked_project() -> (tempfile::TempDir, TemplatePaths) {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        (
            "templates/base.html.twig",
            "<html>\n{% block title %}Site{% endblock %}\n{% block body %}{% endblock %}\n</html>\n",
        ),
        (
            "templates/layout.html.twig",
            "{% extends 'base.html.twig' %}\n{% block body %}<main>{% block content %}{% endblock %}</main>{% endblock %}\n",
        ),
        (
            "templates/forms.html.twig",
            "{% macro input(name) %}<input name=\"{{ name }}\">{% endmacro %}\n{% macro label(text) %}{{ text }}{% endmacro %}\n",
        ),
        ("templates/page.html.twig", "{% extends 'layout.html.twig' %}\n{% include 'forms.html.twig' %}\n"),
    ];
    for (file, text) in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let paths = TemplatePaths::discover(dir.path());
    (dir, paths)
}

/// Goto at the caret of `src`: `(file name, or "" for the template itself,
/// line, the text at the range)`.
fn goto(src: &str, paths: &TemplatePaths) -> Vec<(String, u32, String)> {
    let (text, pos) = caret(src);
    definition(&text, pos, paths)
        .into_iter()
        .map(|target| {
            let (name, source) = match &target.path {
                Some(path) => (
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    std::fs::read_to_string(path).unwrap().replace("\r\n", "\n"),
                ),
                None => (String::new(), text.clone()),
            };
            let (start, end) = LineIndex::new(&source).byte_range(target.range);
            (name, target.range.start.line, source[start as usize..end as usize].to_owned())
        })
        .collect()
}

fn at(file: &str, line: u32, text: &str) -> (String, u32, String) {
    (file.to_owned(), line, text.to_owned())
}

#[test]
fn a_macro_call_goes_to_the_macro() {
    let (_dir, paths) = linked_project();
    // Through an import alias, and a name imported by `from` under another name.
    let found = goto("{% import 'forms.html.twig' as forms %}\n{{ forms.la/*|*/bel('x') }}", &paths);
    assert_eq!(found, [at("forms.html.twig", 1, "label")]);
    let found = goto("{% from 'forms.html.twig' import input as field %}\n{{ fie/*|*/ld('x') }}", &paths);
    assert_eq!(found, [at("forms.html.twig", 0, "input")]);
    // A macro of the template itself.
    let own = "{% macro button(label) %}{% endmacro %}\n";
    assert_eq!(goto(&format!("{own}{{{{ _self.but/*|*/ton('x') }}}}"), &paths), [at("", 0, "button")]);
    assert_eq!(
        goto(&format!("{own}{{% import _self as ui %}}{{{{ ui.but/*|*/ton('x') }}}}"), &paths),
        [at("", 0, "button")]
    );
    // A macro the imported template doesn't have, an attribute of a variable: nowhere.
    assert_eq!(goto("{% import 'forms.html.twig' as forms %}{{ forms.no/*|*/pe() }}", &paths), []);
    assert_eq!(goto("{{ user.na/*|*/me }}", &paths), []);
}

#[test]
fn a_block_name_goes_to_the_block_it_overrides() {
    let (_dir, paths) = linked_project();
    // In the parent, and in the parent's parent.
    let found = goto("{% extends 'layout.html.twig' %}\n{% block con/*|*/tent %}x{% endblock %}", &paths);
    assert_eq!(found, [at("layout.html.twig", 1, "content")]);
    let found = goto("{% extends 'layout.html.twig' %}\n{% block ti/*|*/tle %}x{% endblock %}", &paths);
    assert_eq!(found, [at("base.html.twig", 1, "title")]);
    // A block no parent has, a template without a parent.
    assert_eq!(goto("{% extends 'layout.html.twig' %}{% block ne/*|*/w %}{% endblock %}", &paths), []);
    assert_eq!(goto("{% block ti/*|*/tle %}{% endblock %}", &paths), []);
}

#[test]
fn a_template_name_goes_to_its_file() {
    let (_dir, paths) = linked_project();
    let found = goto("{% extends 'lay/*|*/out.html.twig' %}", &paths);
    assert_eq!(found, [at("layout.html.twig", 0, "")]);
    assert_eq!(goto("{% extends 'no/*|*/ne.html.twig' %}", &paths), []);
}

#[test]
fn usages_of_a_template_name_are_found_in_the_projects_templates() {
    let (dir, paths) = linked_project();
    let found: Vec<(String, u32)> = template_usages("forms.html.twig", &paths)
        .into_iter()
        .map(|(path, range)| (path.file_name().unwrap().to_string_lossy().into_owned(), range.start.line))
        .collect();
    assert_eq!(found, [("page.html.twig".to_owned(), 1)]);
    assert_eq!(template_usages("layout.html.twig", &paths).len(), 1);
    assert_eq!(template_usages("nobody.html.twig", &paths), []);
    // A file's name is what the others call it.
    assert_eq!(paths.name_of(&dir.path().join("templates/forms.html.twig")).as_deref(), Some("forms.html.twig"));
    assert_eq!(paths.name_of(&dir.path().join("src/forms.html.twig")), None);
}

#[test]
fn a_project_has_symfonys_extensions_when_composer_installs_its_bridge() {
    let dir = tempfile::tempdir().unwrap();
    assert!(!TwigProject::discover(dir.path()).symfony);
    std::fs::write(dir.path().join("composer.json"), r#"{"require": {"symfony/twig-bundle": "^7.0"}}"#).unwrap();
    assert!(TwigProject::discover(dir.path()).symfony);
}

// --- Rename --------------------------------------------------------------------

/// A base template, two children and a template of macros with two users.
fn family_project() -> (tempfile::TempDir, TemplatePaths) {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        ("templates/base.html.twig", "{% block body %}{% endblock body %}\n{% block side %}{% endblock %}\n"),
        (
            "templates/page.html.twig",
            "{% extends 'base.html.twig' %}\n{% block body %}{{ block('body') }}{% endblock %}\n",
        ),
        ("templates/other.html.twig", "{% extends 'base.html.twig' %}\n{% block body %}x{% endblock %}\n"),
        ("templates/alone.html.twig", "{% block body %}not of the family{% endblock %}\n"),
        (
            "templates/forms.html.twig",
            "{% macro input(name) %}{{ _self.label(name) }}{% endmacro input %}\n{% macro label(text) %}{% endmacro %}\n",
        ),
        (
            "templates/a.html.twig",
            "{% import 'forms.html.twig' as forms %}\n{{ forms.input('q') }}{{ forms.label('q') }}\n",
        ),
        (
            "templates/b.html.twig",
            "{% from 'forms.html.twig' import input, label as caption %}\n{{ input('q') }}{{ caption('q') }}\n",
        ),
        ("templates/c.html.twig", "{% import 'other.html.twig' as forms %}\n{{ forms.input('q') }}\n"),
    ];
    for (file, text) in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let paths = TemplatePaths::discover(dir.path());
    (dir, paths)
}

/// Renames at the caret of `src`, the template `file` of the project: what
/// is replaced, as `(file name, line, the text replaced)`.
fn renamed(
    dir: &tempfile::TempDir,
    paths: &TemplatePaths,
    file: &str,
    src: &str,
    to: &str,
) -> Vec<(String, u32, String)> {
    let (text, pos) = caret(src);
    let path = dir.path().join("templates").join(file);
    assert!(prepare_rename(&text, pos, &path, paths).is_some(), "nothing to rename in {src}");
    let found = rename(&text, pos, &path, to, paths).expect("something to rename").expect("a valid name");
    let mut out = Vec::new();
    for (changed, ranges) in found {
        let name = changed.file_name().unwrap().to_string_lossy().into_owned();
        let source = if name == file { text.clone() } else { std::fs::read_to_string(&changed).unwrap() };
        let index = LineIndex::new(&source);
        for range in ranges {
            let (start, end) = index.byte_range(range);
            out.push((name.clone(), range.start.line, source[start as usize..end as usize].to_owned()));
        }
    }
    out.sort();
    out
}

#[test]
fn a_block_is_renamed_in_its_family_of_templates() {
    let (dir, paths) = family_project();
    let page = "{% extends 'base.html.twig' %}\n{% block bo/*|*/dy %}{{ block('body') }}{% endblock %}\n";
    let found = renamed(&dir, &paths, "page.html.twig", page, "main");
    assert_eq!(
        found,
        [
            // The parent: the tag and the name after `endblock`.
            at("base.html.twig", 0, "body"),
            at("base.html.twig", 0, "body"),
            // The sibling.
            at("other.html.twig", 1, "body"),
            // This template: the tag and the `block()` call.
            at("page.html.twig", 1, "body"),
            at("page.html.twig", 1, "body"),
        ]
    );
    // Not `side`, and not the block of the same name in a template outside the family.
    assert!(!found.iter().any(|(file, ..)| file == "alone.html.twig"));
}

#[test]
fn a_macro_is_renamed_where_it_is_declared_and_called() {
    let (dir, paths) = family_project();
    let forms = "{% macro in/*|*/put(name) %}{{ _self.label(name) }}{% endmacro input %}\n{% macro label(text) %}{% endmacro %}\n";
    let found = renamed(&dir, &paths, "forms.html.twig", forms, "field");
    assert_eq!(
        found,
        [
            // Through the alias of an import.
            at("a.html.twig", 1, "input"),
            // The name of `from … import`, and its call.
            at("b.html.twig", 0, "input"),
            at("b.html.twig", 1, "input"),
            // The declaration and the name after `endmacro`.
            at("forms.html.twig", 0, "input"),
            at("forms.html.twig", 0, "input"),
        ]
    );
    // `c.html.twig` calls `forms.input` of another template.

    // From a call, and a macro imported under another name: its calls keep that name.
    let a = "{% import 'forms.html.twig' as forms %}\n{{ forms.input('q') }}{{ forms.la/*|*/bel('q') }}\n";
    let found = renamed(&dir, &paths, "a.html.twig", a, "caption");
    assert_eq!(
        found,
        [
            at("a.html.twig", 1, "label"),
            at("b.html.twig", 0, "label"),
            at("forms.html.twig", 0, "label"),
            at("forms.html.twig", 1, "label"),
        ]
    );
}

#[test]
fn what_is_no_block_or_macro_is_not_renamed_and_a_bad_name_is_refused() {
    let (dir, paths) = family_project();
    let path = dir.path().join("templates/page.html.twig");
    for src in ["{% extends 'ba/*|*/se.html.twig' %}", "{{ us/*|*/er.name }}", "{{ name|up/*|*/per }}", "te/*|*/xt"] {
        let (text, pos) = caret(src);
        assert_eq!(prepare_rename(&text, pos, &path, &paths), None, "{src}");
        assert!(rename(&text, pos, &path, "x", &paths).is_none(), "{src}");
    }
    let (text, pos) = caret("{% block bo/*|*/dy %}{% endblock %}");
    assert!(rename(&text, pos, &path, "not a name", &paths).unwrap().is_err());
    assert!(rename(&text, pos, &path, "1st", &paths).unwrap().is_err());
}

/// A card with a `body` block, and a page (child of a layout that has a
/// `body` block too) embedding the card.
fn embedding_project() -> (tempfile::TempDir, TemplatePaths) {
    let dir = tempfile::tempdir().unwrap();
    let files = [
        ("templates/layout.html.twig", "{% block body %}{% endblock %}\n"),
        ("templates/card.html.twig", "<div>{% block body %}{% endblock %}</div>\n"),
        ("templates/fancy.html.twig", "{% extends 'card.html.twig' %}\n{% block body %}fancy{% endblock %}\n"),
        (
            "templates/page.html.twig",
            "{% extends 'layout.html.twig' %}\n{% block body %}\n{% embed 'card.html.twig' %}\n{% block body %}in the card{% endblock %}\n{% endembed %}\n{% endblock %}\n",
        ),
    ];
    for (file, text) in files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let paths = TemplatePaths::discover(dir.path());
    (dir, paths)
}

const EMBEDDING_PAGE: &str = "{% extends 'layout.html.twig' %}\n{% block body %}\n{% embed 'card.html.twig' %}\n{% block body %}in the card{% endblock %}\n{% endembed %}\n{% endblock %}\n";

#[test]
fn a_block_inside_an_embed_is_the_embedded_templates() {
    let (dir, paths) = embedding_project();
    // Renamed from the block inside the embed: the card's family, not the page's.
    let inside = EMBEDDING_PAGE.replace("{% block body %}in the card", "{% block bo/*|*/dy %}in the card");
    assert_eq!(
        renamed(&dir, &paths, "page.html.twig", &inside, "content"),
        [at("card.html.twig", 0, "body"), at("fancy.html.twig", 1, "body"), at("page.html.twig", 3, "body"),]
    );
    // Renamed from the page's own block: the layout's family, and not the block inside the embed.
    let own = EMBEDDING_PAGE.replacen("{% block body %}", "{% block bo/*|*/dy %}", 1);
    assert_eq!(
        renamed(&dir, &paths, "page.html.twig", &own, "main"),
        [at("layout.html.twig", 0, "body"), at("page.html.twig", 1, "body"),]
    );
    // Renamed in the card: the block the page writes inside its embed changes too.
    let card = "<div>{% block bo/*|*/dy %}{% endblock %}</div>\n";
    assert_eq!(
        renamed(&dir, &paths, "card.html.twig", card, "content"),
        [at("card.html.twig", 0, "body"), at("fancy.html.twig", 1, "body"), at("page.html.twig", 3, "body"),]
    );
}

#[test]
fn goto_from_a_block_inside_an_embed_goes_to_the_embedded_template() {
    let (_dir, paths) = embedding_project();
    let inside = EMBEDDING_PAGE.replace("{% block body %}in the card", "{% block bo/*|*/dy %}in the card");
    assert_eq!(goto(&inside, &paths), [at("card.html.twig", 0, "body")]);
    let own = EMBEDDING_PAGE.replacen("{% block body %}", "{% block bo/*|*/dy %}", 1);
    assert_eq!(goto(&own, &paths), [at("layout.html.twig", 0, "body")]);
    // An embed whose template is computed: nowhere to go, nothing to rename.
    let computed = "{% embed name %}{% block bo/*|*/dy %}{% endblock %}{% endembed %}";
    assert_eq!(goto(computed, &paths), []);
    let (text, pos) = caret(computed);
    assert_eq!(prepare_rename(&text, pos, &_dir.path().join("templates/x.html.twig"), &paths), None);
}
