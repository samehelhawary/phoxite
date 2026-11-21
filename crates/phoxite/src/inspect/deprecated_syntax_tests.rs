//! Tests of the deprecated syntax inspection.

use super::*;
use crate::stubs::PhpVersion;

fn project_at(version: &str) -> Project {
    let p = Project::new();
    p.engine.set_stub_filter(StubFilter::all_extensions(PhpVersion::parse(version).unwrap()));
    p.engine.set_inspections(Inspections::new(vec![Arc::new(deprecated_syntax::DeprecatedSyntax)]));
    p
}

/// `(covered text, message)` of each problem.
fn found(text: &str, inspected: &Inspected) -> Vec<(String, String)> {
    inspected.problems.iter().map(|p| (text[p.start as usize..p.end as usize].to_owned(), p.message.clone())).collect()
}

const TEXT: &str = r#"<?php
namespace App;

use Lib\Thing as let;
use Lib\Other as fine;

const is = 1;
const _ = 2;
const LIMIT = 3;
define('OLD', 1, true);
define('NEW', 1);

function readonly() {}
function let() {}
interface is {}

class Report {
    const namespace = 'x';
    const KIND = 'y';

    public function __construct(private int $n) {
        $f = function () { return 1; };
        if ($n) { return; }
        return $n;
    }
    public function __destruct() { yield 1; }
    public function __debugInfo(): ?array { return null; }
    public function build(): int {
        try {
            return 1;
        } finally {
            $g = fn() => 2;
            $h = function () { return 3; };
            return 4;
        }
    }
    public function plain(): array { return []; }
}
enum Level { public function __debugInfo(): array { return []; } }
"#;

#[test]
fn what_php_8_6_deprecates() {
    let mut p = project_at("8.6");
    let inspected = p.inspect(TEXT);
    let since = "is deprecated since PHP 8.6";
    let want: Vec<(String, String)> = [
        ("let", "'let' as the name of an alias"),
        ("is", "'is' as the name of a constant"),
        ("_", "'_' as the name of a constant"),
        ("true", "The third argument of 'define()'"),
        ("readonly", "A function named 'readonly'"),
        ("let", "'let' as the name of a function"),
        ("is", "'is' as the name of an interface"),
        ("namespace", "'namespace' as the name of a class constant"),
        ("$n", "Returning a value from '__construct()'"),
        ("yield", "'__destruct()' as a generator"),
        ("?array", "A nullable return type of '__debugInfo()'"),
        ("return", "'return' in a 'finally' block"),
    ]
    .into_iter()
    .map(|(covered, message)| (covered.to_owned(), format!("{message} {since}")))
    .collect();
    assert_eq!(found(TEXT, &inspected), want);
    assert!(inspected.problems.iter().all(|p| p.severity == Severity::Warning && !p.deprecated));
    // The `return` reported in `finally` is the block's own, not the closure's inside it.
    let finally = inspected.problems.iter().find(|p| p.message.starts_with("'return' in")).unwrap();
    assert!(TEXT[finally.start as usize..].starts_with("return 4;"));
}

#[test]
fn nothing_before_8_6_and_registered() {
    let mut p = project_at("8.5");
    assert!(p.inspect(TEXT).problems.is_empty());
    assert!(Inspections::builtin().ids().contains(&"PhpDeprecatedSyntax"));

    let mut p = project_at("8.6");
    let text = "<?php\n/** @noinspection PhpDeprecatedSyntaxInspection */\nfunction readonly() {}\n";
    assert!(p.inspect(text).problems.is_empty());
}
