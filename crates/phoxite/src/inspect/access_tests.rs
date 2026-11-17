//! Tests of the access inspections.

use super::*;
use crate::stubs::{PhpVersion, StubFilter};

/// `(id, the text covered, message)` of each problem.
fn found(inspected: &Inspected, text: &str) -> Vec<(&'static str, String, String)> {
    inspected
        .problems
        .iter()
        .map(|p| (p.inspection, text[p.start as usize..p.end as usize].to_owned(), p.message.clone()))
        .collect()
}

const MODEL: &str = r#"<?php
namespace App\Model;

class Base
{
    public int $open = 0;
    protected int $guarded = 0;
    private int $hidden = 0;
    public const SEEN = 1;
    protected const KEPT = 2;
    private const OWN = 3;
    public static int $count = 0;

    public function open(): void {}
    protected function guarded(): void {}
    private function hidden(): void {}
    protected static function make(): static { return new static(); }
    protected function step(): void {}
}

class Child extends Base
{
    protected function step(): void {}
}

/** A subclass may make a protected method public. */
class Widened extends Base
{
    public function guarded(): void {}
}

final class Sealed
{
    private int $n = 0;
    protected function inner(): void {}
    private function own(): void {}
}

class Magic
{
    private int $x = 0;
    private function quiet(): void {}
    private static function still(): void {}
    public function __call($name, $arguments) {}
    public static function __callStatic($name, $arguments) {}
    public function __get($name) {}
    public function __set($name, $value) {}
}

/** @property int $virtual */
class Tagged
{
    private int $virtual = 0;
}

trait Helper
{
    private function assist(): void {}
}

final class Uses
{
    use Helper;
}

class Wallet
{
    public private(set) int $balance = 0;
    public protected(set) string $owner = '';
    public readonly int $id;
    private readonly int $pin;
    /** @var list<string> */
    public readonly array $log;

    public function __construct(int $id)
    {
        $this->id = $id;
        $this->pin = 0;
        $this->log = [];
    }
}
"#;

const INACCESSIBLE: &str = "PhpInaccessibleMember";
const READONLY: &str = "PhpReadonlyPropertyWrittenOutsideDeclarationScope";

fn project(mode: TypeCheckMode) -> Project {
    let p = Project::with_files(&[("src/Model.php", MODEL)]);
    p.engine.set_inspections(Inspections::new(vec![
        Arc::new(access::InaccessibleMember { mode }),
        Arc::new(access::ReadonlyWrite { mode }),
    ]));
    p
}

fn private(what: &str, class: &str) -> String {
    format!("Private {what} of 'App\\Model\\{class}' is not accessible here")
}

fn protected(what: &str, class: &str) -> String {
    format!("Protected {what} of 'App\\Model\\{class}' is not accessible here")
}

fn set(name: &str, visibility: &str) -> String {
    format!("Property '${name}' of 'App\\Model\\Wallet' is {visibility}(set): it can't be written here")
}

const OUTSIDE: &str = r#"<?php
namespace App;

use App\Model\{Base, Child, Magic, Sealed, Tagged, Uses, Wallet};

function reads(Base $b, Child $c, Sealed $s, Uses $u): void
{
    // Public members.
    $b->open();
    echo $b->open, Base::SEEN, Base::$count;

    $s->own();
    $s->inner();
    echo $s->n;
    echo Base::OWN, Base::KEPT;
    $b->hidden();
    // A trait's private method belongs to the class using it.
    $u->assist();
    // Nothing of Child's subclasses (it has none) makes this public.
    $c->guarded();
    $c->step();
}

function quiet(Base $b, Magic $m, Tagged $t, Sealed $s, $unknown): void
{
    // `Widened`, a subclass of Base, makes it public: `$b` may be one.
    $b->guarded();
    // Magic answers for what isn't accessible.
    $m->quiet();
    Magic::still();
    echo $m->x;
    $m->x = 1;
    // The `@property` tag stands in for the private property.
    echo $t->virtual;
    $unknown->own();
    // A closure outside a class may be bound to one.
    $bound = function () use ($s) { return $s->n; };
    $arrow = fn () => $s->own();
    // Pest's `test()` is a proxy for the test case it is typed as.
    test()->inner();
}

function test(): Sealed { return new Sealed(); }

function writes(Wallet $w): void
{
    echo $w->balance, $w->owner, $w->id;
    $w->balance = 5;
    $w->owner = 'me';
    $w->balance += 1;
    $w->id = 9;
    $w->log[] = 'x';
    [$w->balance, $other] = [1, 2];
}
"#;

#[test]
fn members_used_from_outside_their_class() {
    let mut p = project(TypeCheckMode::Lenient);
    assert_eq!(
        found(&p.inspect(OUTSIDE), OUTSIDE),
        [
            (INACCESSIBLE, "own".to_owned(), private("method 'own'", "Sealed")),
            (INACCESSIBLE, "inner".into(), protected("method 'inner'", "Sealed")),
            (INACCESSIBLE, "n".into(), private("property '$n'", "Sealed")),
            (INACCESSIBLE, "OWN".into(), private("class constant 'OWN'", "Base")),
            (INACCESSIBLE, "KEPT".into(), protected("class constant 'KEPT'", "Base")),
            (INACCESSIBLE, "hidden".into(), private("method 'hidden'", "Base")),
            (INACCESSIBLE, "assist".into(), private("method 'assist'", "Uses")),
            (INACCESSIBLE, "guarded".into(), protected("method 'guarded'", "Base")),
            (INACCESSIBLE, "step".into(), protected("method 'step'", "Child")),
            (INACCESSIBLE, "balance".into(), set("balance", "private")),
            (INACCESSIBLE, "owner".into(), set("owner", "protected")),
            (INACCESSIBLE, "balance".into(), set("balance", "private")),
            (
                READONLY,
                "id".into(),
                "Readonly property '$id' of 'App\\Model\\Wallet' can only be written in its class and subclasses"
                    .into()
            ),
            (
                READONLY,
                "log".into(),
                "Readonly property '$log' of 'App\\Model\\Wallet' can only be written in its class and subclasses"
                    .into()
            ),
            (INACCESSIBLE, "balance".into(), set("balance", "private")),
        ]
    );
}

const INSIDE: &str = r#"<?php
namespace App;

use App\Model\{Base, Child, Sealed, Wallet};

class Inside extends Base
{
    public function fine(Base $b, Child $c, self $other): void
    {
        $this->guarded();
        $b->guarded();
        parent::guarded();
        static::make();
        echo $this->guarded, $other->guarded, self::KEPT, static::KEPT;
        // A sibling's override of a method their parent declared.
        $c->step();
        $other->guarded = 1;
    }

    public function wrong(Base $b, Sealed $s): void
    {
        $b->hidden();
        $s->inner();
        echo Base::OWN;
        // In a closure: it may be bound to another class.
        $late = fn () => $s->own();
    }

    private int $mine = 0;
    private function only(): void {}

    public function same(Inside $other): int
    {
        $other->only();
        return $other->mine;
    }
}

class Purse extends Wallet
{
    public function refill(Wallet $w): void
    {
        $this->owner = 'mine';
        $this->id = 1;
        $w->id = 2;
        $this->balance = 3;
        $this->pin = 4;
    }
}

// In a trait the code runs in the classes using it; an anonymous class has no name to check against.
trait Peeks
{
    public function peek(Sealed $s): void { $s->own(); }
}

new class extends Base {
    public function peek(Sealed $s, Base $b): void { $s->own(); $b->guarded(); }
};
"#;

#[test]
fn members_used_from_inside_a_class() {
    let mut p = project(TypeCheckMode::Lenient);
    let lenient = found(&p.inspect(INSIDE), INSIDE);
    assert_eq!(
        lenient,
        [
            (INACCESSIBLE, "hidden".to_owned(), private("method 'hidden'", "Base")),
            (INACCESSIBLE, "inner".into(), protected("method 'inner'", "Sealed")),
            (INACCESSIBLE, "OWN".into(), private("class constant 'OWN'", "Base")),
            (INACCESSIBLE, "balance".into(), set("balance", "private")),
        ]
    );

    // Strict: what may fail too.
    let mut p = project(TypeCheckMode::Strict);
    let strict = found(&p.inspect(INSIDE), INSIDE);
    let added: Vec<_> = strict.into_iter().filter(|f| !lenient.contains(f)).collect();
    assert_eq!(added, [(INACCESSIBLE, "own".to_owned(), private("method 'own'", "Sealed"))]);
}

#[test]
fn a_union_receiver_is_reported_in_strict_mode() {
    let text = "<?php\nnamespace App;\n\nuse App\\Model\\{Base, Sealed};\n\nfunction f(Sealed|Base $either): void\n{\n    $either->inner();\n}\n";
    let mut p = project(TypeCheckMode::Lenient);
    assert!(found(&p.inspect(text), text).is_empty());
    let mut p = project(TypeCheckMode::Strict);
    assert_eq!(
        found(&p.inspect(text), text),
        [(INACCESSIBLE, "inner".to_owned(), protected("method 'inner'", "Sealed"))]
    );
}

/// Before PHP 8.4 only the declaring class initialises a readonly property;
/// from 8.4 it is `protected(set)`.
#[test]
fn readonly_properties_by_php_version() {
    let text = r#"<?php
namespace App;

use App\Model\Wallet;

class Purse extends Wallet
{
    public function relabel(): void { $this->id = 1; }
}
"#;
    let mut p = project(TypeCheckMode::Lenient);
    assert!(found(&p.inspect(text), text).is_empty(), "the latest PHP");

    let p83 = project(TypeCheckMode::Lenient);
    p83.engine.set_stub_filter(StubFilter::all_extensions(PhpVersion::new(8, 3)));
    let mut p83 = p83;
    assert_eq!(
        found(&p83.inspect(text), text),
        [(
            READONLY,
            "id".to_owned(),
            "Readonly property '$id' of 'App\\Model\\Wallet' can only be written in its class".to_owned()
        )]
    );
}

#[test]
fn registered_suppressible_and_quiet_in_dumb_mode() {
    let ids = Inspections::builtin().ids();
    assert!(ids.contains(&INACCESSIBLE) && ids.contains(&READONLY), "{ids:?}");

    let text = "<?php\nnamespace App;\n\nuse App\\Model\\Sealed;\n\nfunction f(Sealed $s): void\n{\n    /** @noinspection PhpInaccessibleMemberInspection */\n    $s->own();\n    $s->inner();\n}\n";
    let mut p = project(TypeCheckMode::Lenient);
    let problems = found(&p.inspect(text), text);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].1, "inner");

    p.engine.shutdown();
    assert!(found(&p.inspect(text), text).is_empty());
}
