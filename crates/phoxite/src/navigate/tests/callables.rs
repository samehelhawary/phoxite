use super::*;
use crate::navigate::UsageKind;

const LIB: &str = r"<?php
namespace App;

function by_name(array $a, array $b): int { return 0; }

class Sorter
{
    public static function make(): self { return new self(); }
    public function compare(array $a, array $b): int { return 0; }
    public function sort(array $rows): array
    {
        usort($rows, [$this, 'compare']);
        return $rows;
    }
}

/** @param callable $then */
function later($then): void {}
function column(callable|string $of): void {}
function either(callable|array $of): void {}
";

const USER: &str = r"<?php
namespace Web;

use App\Sorter;

function run(Sorter $sorter, array $rows, $unknown): void
{
    usort($rows, 'App\by_name');
    usort($rows, '\\App\\by_name');
    usort($rows, [$sorter, 'compare']);
    usort($rows, [Sorter::class, 'compare']);
    usort($rows, ['App\Sorter', 'compare']);
    \App\later('App\Sorter::make');
    \App\later(then: 'App\by_name');
    \App\column('App\by_name');
    \App\either([$sorter, 'compare']);
    usort($rows, [$unknown, 'compare']);
    usort($rows, 'App\nothing');
    usort($rows, [$sorter, 'nothing']);
    echo 'App\by_name';
}
";

impl Project {
    fn usages(&mut self, rel: &str, source: &str) -> Vec<(String, u32, String, UsageKind)> {
        let (text, pos) = caret(source);
        let doc = self.open(rel, &text);
        let usages = self.engine.references(doc, pos, false, &CancellationToken::new());
        let found = usages
            .expect("something at the caret")
            .unwrap()
            .iter()
            .map(|usage| {
                let (rel, line, text) = self.describe(&usage.path, usage.range);
                (rel, line, text, usage.kind)
            })
            .collect();
        self.engine.document_closed(doc);
        found
    }
}

fn project() -> Project {
    Project::new(&[("src/lib.php", LIB), ("src/user.php", USER)])
}

fn string(rel: &str, line: u32, text: &str) -> (String, u32, String, UsageKind) {
    (rel.to_owned(), line, text.to_owned(), UsageKind::String)
}

#[test]
fn a_string_where_a_callable_is_expected_goes_to_what_it_names() {
    let mut project = project();
    let at = |project: &mut Project, from: &str, to: &str| project.definition("src/user.php", &USER.replace(from, to));
    let function = Some(vec![target("src/lib.php", 3, "by_name")]);
    let method = Some(vec![target("src/lib.php", 8, "compare")]);

    assert_eq!(at(&mut project, "usort($rows, 'App\\by_name');", "usort($rows, 'App\\by_/*|*/name');"), function);
    // Written with escaped backslashes and a leading one.
    assert_eq!(at(&mut project, r"'\\App\\by_name'", r"'\\App\\by_/*|*/name'"), function);
    // A parameter only its docblock types, by a named argument.
    assert_eq!(at(&mut project, "then: 'App\\by_name'", "then: 'App\\by_/*|*/name'"), function);
    // The method of an object, of `Foo::class` and of a class's name.
    assert_eq!(at(&mut project, "usort($rows, [$sorter, 'compare']", "usort($rows, [$sorter, 'com/*|*/pare']"), method);
    assert_eq!(at(&mut project, "[Sorter::class, 'compare']", "[Sorter::class, 'com/*|*/pare']"), method);
    assert_eq!(at(&mut project, "['App\\Sorter', 'compare']", "['App\\Sorter', 'com/*|*/pare']"), method);
    // `'Class::method'`: the method, and the class before it.
    assert_eq!(
        at(&mut project, "'App\\Sorter::make'", "'App\\Sorter::ma/*|*/ke'"),
        Some(vec![target("src/lib.php", 7, "make")])
    );
    assert_eq!(
        at(&mut project, "'App\\Sorter::make'", "'App\\Sor/*|*/ter::make'"),
        Some(vec![target("src/lib.php", 5, "Sorter")])
    );

    // Not where a plain string or array is as good, nor a name of nothing, nor any other string.
    for (from, to) in [
        ("column('App\\by_name')", "column('App\\by_/*|*/name')"),
        ("either([$sorter, 'compare'])", "either([$sorter, 'com/*|*/pare'])"),
        ("[$unknown, 'compare']", "[$unknown, 'com/*|*/pare']"),
        ("'App\\nothing'", "'App\\not/*|*/hing'"),
        ("[$sorter, 'nothing']", "[$sorter, 'not/*|*/hing']"),
        ("echo 'App\\by_name'", "echo 'App\\by_/*|*/name'"),
    ] {
        let found = at(&mut project, from, to);
        assert!(found.as_ref().is_none_or(Vec::is_empty), "{to}: {found:?}");
    }
}

#[test]
fn a_callable_a_function_returns_or_a_property_is_assigned() {
    let holder = r"<?php
namespace App;

class Holder
{
    /** @var callable */
    public $then;
    public array $rows = [];

    public function sorter(): callable { return [$this, 'compare']; }
    /** @return callable */
    public function other() { return 'App\by_name'; }
    public function plain(): array { return [$this, 'compare']; }
    public function compare(array $a, array $b): int { return 0; }
    public function set(): void
    {
        $this->then = 'App\by_name';
        $this->then = [$this, 'compare'];
        $this->rows = [$this, 'compare'];
        $local = 'App\by_name';
    }
}

function pick(): callable { return 'App\by_name'; }
";
    let mut project = Project::new(&[("src/lib.php", LIB), ("src/holder.php", holder)]);
    let at = |project: &mut Project, from: &str, to: &str| {
        assert!(holder.contains(from), "{from}");
        project.definition("src/holder.php", &holder.replace(from, to))
    };
    let function = Some(vec![target("src/lib.php", 3, "by_name")]);
    let method = Some(vec![target("src/holder.php", 13, "compare")]);
    assert_eq!(
        at(&mut project, "callable { return [$this, 'compare']", "callable { return [$this, 'com/*|*/pare']"),
        method
    );
    assert_eq!(at(&mut project, "other() { return 'App\\by_name'", "other() { return 'App\\by_/*|*/name'"), function);
    assert_eq!(
        at(&mut project, "pick(): callable { return 'App\\by_name'", "pick(): callable { return 'App\\by_/*|*/name'"),
        function
    );
    assert_eq!(at(&mut project, "$this->then = 'App\\by_name'", "$this->then = 'App\\by_/*|*/name'"), function);
    assert_eq!(at(&mut project, "$this->then = [$this, 'compare']", "$this->then = [$this, 'com/*|*/pare']"), method);
    // Not what an array is returned as or assigned to, nor a variable's value.
    for (from, to) in [
        ("array { return [$this, 'compare']", "array { return [$this, 'com/*|*/pare']"),
        ("$this->rows = [$this, 'compare']", "$this->rows = [$this, 'com/*|*/pare']"),
        ("$local = 'App\\by_name'", "$local = 'App\\by_/*|*/name'"),
    ] {
        let found = at(&mut project, from, to);
        assert!(found.as_ref().is_none_or(Vec::is_empty), "{to}: {found:?}");
    }

    let found = project.usages("src/holder.php", &holder.replace("function compare", "function com/*|*/pare"));
    assert_eq!(found, [string("src/holder.php", 9, "compare"), string("src/holder.php", 17, "compare")]);
    let found = project.usages("src/lib.php", &LIB.replace("function by_name", "function by_/*|*/name"));
    assert_eq!(
        found,
        [
            string("src/holder.php", 11, r"App\by_name"),
            string("src/holder.php", 16, r"App\by_name"),
            string("src/holder.php", 23, r"App\by_name"),
        ]
    );
}

#[test]
fn string_callables_are_usages_of_what_they_name() {
    let mut project = project();
    let found = project.usages("src/lib.php", &LIB.replace("function by_name", "function by_/*|*/name"));
    assert_eq!(
        found,
        [
            string("src/user.php", 7, r"App\by_name"),
            string("src/user.php", 8, r"\\App\\by_name"),
            string("src/user.php", 13, r"App\by_name"),
        ]
    );

    let found = project.usages("src/lib.php", &LIB.replace("function compare", "function com/*|*/pare"));
    assert_eq!(
        found,
        [
            string("src/lib.php", 11, "compare"),
            string("src/user.php", 9, "compare"),
            string("src/user.php", 10, "compare"),
            string("src/user.php", 11, "compare"),
        ]
    );

    // From the string itself, and the class of `'Class::method'`.
    let found = project.usages("src/user.php", &USER.replace("'App\\Sorter::make'", "'App\\Sorter::ma/*|*/ke'"));
    assert_eq!(found, [string("src/user.php", 12, "make")]);
    let found = project.usages("src/lib.php", &LIB.replace("class Sorter", "class Sor/*|*/ter"));
    let strings: Vec<_> = found.into_iter().filter(|usage| usage.3 == UsageKind::String).collect();
    assert_eq!(strings, [string("src/user.php", 11, r"App\Sorter"), string("src/user.php", 12, r"App\Sorter")]);
}

#[test]
fn renaming_follows_string_callables() {
    let mut project = project();
    let renamed = |project: &mut Project, rel: &str, source: &str, name: &str| {
        let (text, pos) = caret(source);
        let doc = project.open(rel, &text);
        let edit = project.engine.rename(doc, pos, name, &CancellationToken::new()).expect("handled").unwrap();
        project.engine.document_closed(doc);
        let mut lines = Vec::new();
        for file in edit.files {
            let crate::base::FileEdit::Edit { path, edits, .. } = file else { continue };
            for edit in edits {
                let (rel, line, text) = project.describe(&path, edit.range);
                lines.push((rel, line, text, edit.new_text));
            }
        }
        lines.sort();
        lines
    };
    let line = |rel: &str, line: u32, old: &str, new: &str| (rel.to_owned(), line, old.to_owned(), new.to_owned());

    // From the string: the declaration and every string, the namespace kept.
    let found = renamed(
        &mut project,
        "src/user.php",
        &USER.replace("usort($rows, 'App\\by_name');", "usort($rows, 'App\\by_/*|*/name');"),
        "by_title",
    );
    assert_eq!(
        found,
        [
            line("src/lib.php", 3, "by_name", "by_title"),
            line("src/user.php", 7, "by_name", "by_title"),
            line("src/user.php", 8, "by_name", "by_title"),
            line("src/user.php", 13, "by_name", "by_title"),
        ]
    );
    let found =
        renamed(&mut project, "src/lib.php", &LIB.replace("function compare", "function com/*|*/pare"), "order");
    assert_eq!(
        found,
        [
            line("src/lib.php", 8, "compare", "order"),
            line("src/lib.php", 11, "compare", "order"),
            line("src/user.php", 9, "compare", "order"),
            line("src/user.php", 10, "compare", "order"),
            line("src/user.php", 11, "compare", "order"),
        ]
    );
}
