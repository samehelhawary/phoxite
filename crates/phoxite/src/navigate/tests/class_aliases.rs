use super::*;

const REAL: &str = r"<?php
namespace Acme\Orm\Fields;

class ScalarField
{
    public function getName(): string { return ''; }
}

class BooleanField extends ScalarField {}
";

const COMPATIBILITY: &str = r"<?php
namespace Acme\Compat;

class_alias('Acme\Orm\Fields\ScalarField', 'Acme\Entity\ScalarField');
class_alias(\Acme\Orm\Fields\BooleanField::class, '\Acme\Entity\BooleanField');
";

const AUTOLOAD: &str = r"<?php

use Acme\Orm\Fields\ScalarField;

class_alias('\Acme\Orm\Fields\ScalarField', '\Shop\Field');
class_alias(ScalarField::class, 'Legacy_Field');
// An alias of an alias.
class_alias('Legacy_Field', 'Older_Field');
// A loop declares nothing a class lookup can reach.
class_alias('Loop\A', 'Loop\B');
class_alias('Loop\B', 'Loop\A');
// Put together at run time: nothing.
class_alias($class, 'Dynamic\Alias');
";

const USER: &str = r"<?php
namespace Shop\Blocks;

use Acme\Entity;

class DataManager
{
    public function check($field, Entity\ScalarField $typed): void
    {
        if ($field instanceof Entity\ScalarField) {}
        if ($field instanceof Entity\BooleanField) {}
        if ($field instanceof \Shop\Field) {}
        if ($field instanceof \Older_Field) {}
        echo $typed->getName();
    }
}
";

const FIELDS: &str = "lib/orm/fields.php";
const HERE: &str = "lib/blocks/datamanager.php";

fn project() -> Project {
    Project::new(&[
        (FIELDS, REAL),
        ("include/compatibility.php", COMPATIBILITY),
        ("include/autoload.php", AUTOLOAD),
        (HERE, USER),
    ])
}

#[test]
fn an_alias_goes_to_the_class_it_stands_for() {
    let mut project = project();
    let mut at = |from: &str, to: &str| project.definition(HERE, &USER.replacen(from, to, 1));
    let scalar = Some(vec![target(FIELDS, 3, "ScalarField")]);

    // Through a `use`d namespace.
    assert_eq!(at("instanceof Entity\\ScalarField", "instanceof Entity\\Scalar/*|*/Field"), scalar);
    // A class written as `Foo::class`, an alias with a leading `\`.
    assert_eq!(
        at("instanceof Entity\\BooleanField", "instanceof Entity\\Boolean/*|*/Field"),
        Some(vec![target(FIELDS, 8, "BooleanField")])
    );
    // `class_alias()` with leading `\`s.
    assert_eq!(at("\\Shop\\Field", "\\Shop\\Fi/*|*/eld"), scalar);
    // An alias of an alias (of `Foo::class`).
    assert_eq!(at("\\Older_Field", "\\Older_/*|*/Field"), scalar);
}

#[test]
fn an_alias_has_the_members_of_its_class() {
    let mut project = project();
    let found = project.definition(HERE, &USER.replace("$typed->getName()", "$typed->get/*|*/Name()"));
    assert_eq!(found, Some(vec![target(FIELDS, 5, "getName")]));
}

#[test]
fn only_written_aliases_are_declared() {
    let project = project();
    let declares = |fqn: &str| project.engine.declares(NameKind::ClassLike, fqn);
    assert!(declares("Acme\\Entity\\ScalarField"));
    assert!(declares("acme\\entity\\booleanfield"));
    assert!(declares("Shop\\Field"));
    assert!(declares("Older_Field"));
    assert!(!declares("Loop\\A"));
    assert!(!declares("Dynamic\\Alias"));
    assert_eq!(project.engine.class_alias("Loop\\B"), None);
    assert_eq!(project.engine.class_alias("Older_Field").as_deref(), Some("Acme\\Orm\\Fields\\ScalarField"));
}

#[test]
fn a_declared_class_wins_over_an_alias_of_its_name() {
    let mut project = Project::new(&[
        (FIELDS, REAL),
        ("src/alias.php", "<?php\nclass_alias('Acme\\Orm\\Fields\\ScalarField', 'App\\Field');\n"),
        ("src/Field.php", "<?php\nnamespace App;\n\nclass Field {}\n"),
    ]);
    let found = project.definition("src/use.php", "<?php\nnew \\App\\Fi/*|*/eld();\n");
    assert_eq!(found, Some(vec![target("src/Field.php", 3, "Field")]));
}

#[test]
fn a_namespaced_class_alias_function_declares_nothing() {
    let project = Project::new(&[
        (FIELDS, REAL),
        (
            "src/alias.php",
            "<?php\nnamespace Mine;\n\nOwn\\class_alias('Acme\\Orm\\Fields\\ScalarField', 'Mine\\Nope');\n\\class_alias('Acme\\Orm\\Fields\\ScalarField', 'Mine\\Yes');\nclass_alias('Acme\\Orm\\Fields\\ScalarField', 'Mine\\Also');\n",
        ),
    ]);
    assert_eq!(project.engine.class_alias("Mine\\Nope"), None);
    assert!(project.engine.class_alias("Mine\\Yes").is_some());
    assert!(project.engine.class_alias("Mine\\Also").is_some());
}

#[test]
fn an_alias_in_an_open_buffer_counts_and_goes_away_with_it() {
    let mut project = Project::new(&[(FIELDS, REAL), ("src/alias.php", "<?php\n")]);
    assert_eq!(project.engine.class_alias("Fresh\\Alias"), None);
    let doc = project.open("src/alias.php", "<?php\nclass_alias('Acme\\Orm\\Fields\\ScalarField', 'Fresh\\Alias');\n");
    assert_eq!(project.engine.class_alias("Fresh\\Alias").as_deref(), Some("Acme\\Orm\\Fields\\ScalarField"));
    project.engine.document_closed(doc);
    assert_eq!(project.engine.class_alias("Fresh\\Alias"), None);
}

#[test]
fn the_project_s_alias_wins_over_a_library_s() {
    use crate::frameworks::names::ClassAliases;
    use crate::summary::{DeclaredKind, DeclaredName, NameFacts};
    let facts = |class: &str| NameFacts {
        declared: vec![DeclaredName {
            kind: DeclaredKind::ClassAlias,
            name: "Shop\\Field".to_owned(),
            dynamic: false,
            loc: Default::default(),
            params: Vec::new(),
            classes: vec![class.to_owned()],
        }],
        ..NameFacts::default()
    };
    let (library, own, other) = (facts("Vendor\\Field"), facts("Own\\Field"), facts("Other\\Field"));
    let aliases = ClassAliases::new([(&library, true), (&own, false), (&other, false)]);
    assert_eq!(aliases.resolve("\\shop\\field").as_deref(), Some("Own\\Field"));
    let aliases = ClassAliases::new([(&own, false), (&library, true)]);
    assert_eq!(aliases.resolve("Shop\\Field").as_deref(), Some("Own\\Field"));
}

/// A rename started on an alias is refused: its name is declared by a string of
/// `class_alias()`, and renaming would reach for the class it stands for.
#[test]
fn an_alias_is_not_renamed() {
    let mut project = project();
    for (from, to) in [
        (r"instanceof \Shop\Field", r"instanceof \Shop\Fi/*|*/eld"),
        (r"Entity\ScalarField $typed", r"Entity\Scalar/*|*/Field $typed"),
    ] {
        let (text, pos) = caret(&USER.replacen(from, to, 1));
        let doc = project.open(HERE, &text);
        assert_eq!(project.engine.prepare_rename(doc, pos), Some(None), "{to}");
        let renamed = project.engine.rename(doc, pos, "Renamed", &CancellationToken::new()).unwrap();
        assert!(renamed.is_err(), "{to}: {renamed:?}");
        project.engine.document_closed(doc);
    }
    // The files are as they were.
    assert_eq!(std::fs::read_to_string(project.root.join(FIELDS)).unwrap(), REAL);
}

/// The usages of a class are also where an alias writes it, and those of an alias
/// are the class's; a rename of the class leaves code written with an alias alone.
#[test]
fn usages_reach_through_aliases_and_a_rename_does_not() {
    let mut project = project();
    let lines = |project: &mut Project, source: &str| -> Vec<(String, u32)> {
        let (text, pos) = caret(source);
        let doc = project.open(HERE, &text);
        let usages = project.engine.references(doc, pos, false, &CancellationToken::new()).unwrap().unwrap();
        project.engine.document_closed(doc);
        let mut out: Vec<(String, u32)> = usages
            .iter()
            .map(|usage| {
                let rel = usage.path.strip_prefix(&project.root).unwrap().to_string_lossy().replace('\\', "/");
                (rel, usage.range.start.line)
            })
            .collect();
        out.sort();
        out
    };
    // From the real class (its declaration): the aliases' uses in the user's file are
    // among its usages, beside the `class_alias()` arguments naming it.
    let from_class = {
        let (text, pos) = caret(&REAL.replacen("class ScalarField", "class Scalar/*|*/Field", 1));
        let doc = project.open(FIELDS, &text);
        let found = project.engine.references(doc, pos, false, &CancellationToken::new()).unwrap().unwrap();
        project.engine.document_closed(doc);
        found
            .iter()
            .filter(|usage| usage.path.ends_with("datamanager.php"))
            .map(|usage| usage.range.start.line)
            .collect::<Vec<u32>>()
    };
    // Lines from 0: `Entity\ScalarField $typed` (7), the `instanceof Entity\ScalarField` (9),
    // `\Shop\Field` (11) and `\Older_Field` (12), an alias of an alias.
    for line in [7, 9, 11, 12] {
        assert!(from_class.contains(&line), "line {line} in {from_class:?}");
    }
    // Not `Entity\BooleanField`: another class.
    assert!(!from_class.contains(&10), "{from_class:?}");

    // From an alias: the same places.
    let from_alias = lines(&mut project, &USER.replacen(r"instanceof \Shop\Field", r"instanceof \Shop\Fi/*|*/eld", 1));
    let here: Vec<u32> = from_alias.iter().filter(|(rel, _)| rel == HERE).map(|(_, line)| *line).collect();
    for line in [7, 9, 11, 12] {
        assert!(here.contains(&line), "line {line} in {from_alias:?}");
    }

    // A rename of the class doesn't touch what an alias writes, nor the alias's own
    // name; the class's name in `class_alias()` follows, as a string or `Foo::class`.
    let (text, pos) = caret(&REAL.replacen("class ScalarField", "class Scalar/*|*/Field", 1));
    let doc = project.open(FIELDS, &text);
    let edit = project.engine.rename(doc, pos, "PlainField", &CancellationToken::new()).unwrap().unwrap();
    project.engine.document_closed(doc);
    let files = project.apply(&edit);
    assert!(!files.contains_key(HERE), "{:?}", files.get(HERE));
    assert_eq!(
        files["include/compatibility.php"],
        COMPATIBILITY
            .replace(r"class_alias('Acme\Orm\Fields\ScalarField'", r"class_alias('Acme\Orm\Fields\PlainField'")
    );
    let autoload = &files["include/autoload.php"];
    assert!(autoload.contains(r"use Acme\Orm\Fields\PlainField;"), "{autoload}");
    assert!(autoload.contains(r"class_alias('\Acme\Orm\Fields\PlainField', '\Shop\Field');"), "{autoload}");
    assert!(autoload.contains("class_alias(PlainField::class, 'Legacy_Field');"), "{autoload}");
}
