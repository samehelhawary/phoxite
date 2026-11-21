use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::base::settings::{ArrayShapeHints, InlayHintSettings};
use crate::base::{IdeEvent, Range, SharedSink};

use super::*;
use crate::engine::{CACHE_SUBDIR, EngineConfig};
use crate::project::LayoutConfig;
use crate::stubs::StubFilter;
use crate::syntax::LineIndex;

const WAIT: Duration = Duration::from_secs(60);
const CARET: &str = "/*|*/";

fn caret(source: &str) -> (String, Pos) {
    let at = source.find(CARET).expect("a /*|*/ caret");
    let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
    let pos = LineIndex::new(&text).pos(at as u32);
    (text, pos)
}

struct Project {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    engine: Arc<PhpEngine>,
    next_doc: u32,
}

const MODEL: &str = r#"<?php
namespace App;

final class Book
{
    public function __construct(public string $isbn, public string $title = '') {}

    public static function make(string $isbn): static { return new static($isbn); }

    public function withAvailability(bool $available): Book { return $this; }

    public function setTitle(string $title): void {}
}

class Library
{
    /**
     * Lends a book out.
     */
    public function checkout(string $isbn): Book { return new Book($isbn); }
}

/**
 * Formats a price.
 */
function fmt(int $cents, string $currency = 'EUR'): string { return ''; }

function total(int ...$amounts): int { return 0; }

function pick(int $a, int $b): int { return $a; }
"#;

impl Project {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let base = dunce::canonicalize(tmp.path()).unwrap();
        let root = base.join("project");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/Model.php"), MODEL).unwrap();
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let config = EngineConfig {
            cache_dir: base.join("cache").join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(WAIT));
        Self { _tmp: tmp, root, engine, next_doc: 1 }
    }

    fn open(&mut self, text: &str) -> DocId {
        let doc = DocId(self.next_doc);
        self.next_doc += 1;
        self.engine.document_opened(doc, &self.root.join("src/Use.php"), 1, text);
        doc
    }

    /// Signature help at the caret of `body` (after a namespace header).
    fn help(&mut self, body: &str) -> Option<SignatureHelpInfo> {
        let (text, pos) = caret(&format!("<?php\nnamespace App;\n\n{body}"));
        let doc = self.open(&text);
        let found = self.engine.signature_help(doc, pos);
        self.engine.document_closed(doc);
        found
    }

    /// The label and the active parameter's text.
    fn active(&mut self, body: &str) -> (String, Option<String>) {
        let help = self.help(body).unwrap_or_else(|| panic!("signature help in {body:?}"));
        let signature = &help.signatures[0];
        let utf16: Vec<u16> = signature.label.encode_utf16().collect();
        let active = signature
            .parameters
            .get(help.active_parameter as usize)
            .map(|(start, end)| String::from_utf16(&utf16[*start as usize..*end as usize]).unwrap());
        (signature.label.clone(), active)
    }

    /// `text` with the hints of `range` (the whole file when `None`) written
    /// in as `⟨label⟩`.
    fn hints(&mut self, text: &str, range: Option<Range>) -> String {
        self.hints_with(text, range, InlayHintSettings::default())
    }

    /// [`hints`](Self::hints), shown as `settings` say.
    fn hints_with(&mut self, text: &str, range: Option<Range>, settings: InlayHintSettings) -> String {
        let hints = self.hint_infos(text, range, settings);
        let lines = LineIndex::new(text);
        let mut out = text.to_owned();
        for hint in hints.iter().rev() {
            let at = lines.byte(hint.position) as usize;
            out.insert_str(at, &format!("⟨{}⟩", hint.label));
        }
        out
    }

    fn hint_infos(&mut self, text: &str, range: Option<Range>, settings: InlayHintSettings) -> Vec<InlayHintInfo> {
        let doc = self.open(text);
        let lines = LineIndex::new(text);
        let range = range.unwrap_or_else(|| Range::new(Pos::new(0, 0), lines.pos(text.len() as u32)));
        let hints = self.engine.inlay_hints_with(doc, range, settings).expect("handled");
        self.engine.document_closed(doc);
        hints
    }
}

#[test]
fn signature_of_an_unfinished_method_call() {
    let mut p = Project::new();
    let (label, active) = p.active("$lib = new Library();\n$lib->checkout(/*|*/\necho 1;\n");
    assert_eq!(label, r"checkout(string $isbn): App\Book");
    assert_eq!(active.as_deref(), Some("string $isbn"));
    let help = p.help("$lib = new Library();\n$lib->checkout(/*|*/\n").unwrap();
    assert_eq!(help.signatures[0].documentation.as_deref(), Some("Lends a book out."));
}

#[test]
fn active_parameter_by_position_name_and_variadic() {
    let mut p = Project::new();
    let (label, active) = p.active("fmt(1, /*|*/);\n");
    assert_eq!(label, "fmt(int $cents, string $currency = 'EUR'): string");
    assert_eq!(active.as_deref(), Some("string $currency = 'EUR'"));
    assert_eq!(p.active("fmt(/*|*/1, 'x');\n").1.as_deref(), Some("int $cents"));
    // A named argument, finished or being typed.
    assert_eq!(p.active("fmt(currency: /*|*/\n").1.as_deref(), Some("string $currency = 'EUR'"));
    assert_eq!(p.active("fmt(currency: 'x', cents: 1/*|*/);\n").1.as_deref(), Some("int $cents"));
    // Inside a string argument.
    assert_eq!(p.active("fmt(1, 'US/*|*/\n").1.as_deref(), Some("string $currency = 'EUR'"));
    // The rest goes to the variadic parameter.
    assert_eq!(p.active("total(1, 2, 3, /*|*/\n").1.as_deref(), Some("int ...$amounts"));
    // Past the last parameter: none is active.
    assert_eq!(p.active("pick(1, 2, /*|*/);\n").1, None);
    // An operand being typed after an operator.
    assert_eq!(p.active("fmt(1 + /*|*/\n").1.as_deref(), Some("int $cents"));
}

#[test]
fn innermost_call_and_constructors() {
    let mut p = Project::new();
    // The nested call's arguments, then the outer call's again.
    assert_eq!(p.active("fmt(strlen(/*|*/), 'x');\n").0, "strlen(string $string): non-negative-int");
    assert_eq!(p.active("fmt(strlen('ab'), /*|*/);\n").0, "fmt(int $cents, string $currency = 'EUR'): string");
    assert_eq!(p.active("fmt([1, /*|*/\n").1.as_deref(), Some("int $cents"));
    let (label, active) = p.active("$b = new Book('1', /*|*/\n");
    assert_eq!(label, "Book(string $isbn, string $title = '')");
    assert_eq!(active.as_deref(), Some("string $title = ''"));
    assert_eq!(p.active("Book::make(/*|*/\n").0, r"make(string $isbn): App\Book");
}

#[test]
fn no_signature_outside_call_arguments() {
    let mut p = Project::new();
    assert_eq!(p.help("if (/*|*/\n"), None);
    assert_eq!(p.help("function f(/*|*/) {}\n"), None);
    assert_eq!(p.help("$f = fn(/*|*/) => 1;\n"), None);
    assert_eq!(p.help("fmt(1, function () { /*|*/ });\n"), None);
    assert_eq!(p.help("fmt(1); /*|*/\n"), None);
    assert_eq!(p.help("// fmt(/*|*/\n"), None);
    assert_eq!(p.help("unknown_function(/*|*/);\n"), None);
}

#[test]
fn open_call_scans_the_text() {
    let call = |source: &str| {
        let (text, pos) = caret(source);
        let offset = LineIndex::new(&text).byte(pos) as usize;
        open_call(text.as_bytes(), offset)
    };
    let found = call("<?php f(1, ')', [2, 3], g(4), /*|*/").unwrap();
    assert_eq!((found.paren, found.commas, found.closers.as_deref()), (7, 4, Some(")")));
    let found = call("<?php f(\"a, (b\" . /*|*/ 'c');").unwrap();
    assert_eq!((found.commas, found.closers, found.operand_missing), (0, None, true));
    assert_eq!(call("<?php foreach (/*|*/"), None);
    assert_eq!(call("<?php f(1); g(2) /*|*/"), None);
    assert!(call("<?php $f(/*|*/").is_some());
}

#[test]
fn parameter_name_hints() {
    let mut p = Project::new();
    let text = r#"<?php
namespace App;

$book = new Book('978', 'Dune');
$book->withAvailability(false);
fmt(100, 'USD');
fmt($cents, currency: 'USD');
$book->setTitle('Dune');
strlen('abc');
pick(1, 2);
total(1, 2, 3);
fmt(...$args);
fmt(-5, Book::class);
"#;
    let want = r#"<?php
namespace App;

$book = new Book(⟨isbn:⟩'978', ⟨title:⟩'Dune');
$book->withAvailability(⟨available:⟩false);
fmt(⟨cents:⟩100, ⟨currency:⟩'USD');
fmt($cents, currency: 'USD');
$book->setTitle('Dune');
strlen('abc');
pick(1, 2);
total(⟨...amounts:⟩1, 2, 3);
fmt(...$args);
fmt(⟨cents:⟩-5, ⟨currency:⟩Book::class);
"#;
    assert_eq!(p.hints(text, None), want);
}

#[test]
fn type_hints_for_variables_and_closures() {
    let mut p = Project::new();
    let text = r#"<?php
namespace App;

function run(Library $lib, array $books): void
{
    $lent = $lib->checkout('1');
    $lent = $lib->checkout('2');
    $n = 1;
    $b = new Book('1');
    $made = Book::make('1');
    $copy = $lent;
    $title = $lent->title;
    $f = fn (Book $b) => $b->title;
    $g = function (Book $b) use ($n) { return $n; };
    $h = function (): int { return 1; };
    $v = function () { echo 1; };
    /** @var list<Book> $books */
    foreach ([$lent] as $i => $item) {}
}
"#;
    let want = r#"<?php
namespace App;

function run(Library $lib, array $books): void
{
    $lent⟨: Book⟩ = $lib->checkout(⟨isbn:⟩'1');
    $lent = $lib->checkout(⟨isbn:⟩'2');
    $n = 1;
    $b = new Book(⟨isbn:⟩'1');
    $made = Book::make(⟨isbn:⟩'1');
    $copy⟨: Book⟩ = $lent;
    $title⟨: string⟩ = $lent->title;
    $f = fn (Book $b)⟨: string⟩ => $b->title;
    $g = function (Book $b) use ($n)⟨: int⟩ { return $n; };
    $h = function (): int { return 1; };
    $v = function () { echo 1; };
    /** @var list<Book> $books */
    foreach ([$lent] as $i⟨: int⟩ => $item⟨: Book⟩) {}
}
"#;
    assert_eq!(p.hints(text, None), want);
}

#[test]
fn hints_only_in_the_range() {
    let mut p = Project::new();
    let text = "<?php\nnamespace App;\n\nfmt(1, 'a');\nfmt(2, 'b');\n$x = (new Library())->checkout('3');\n";
    let range = Range::new(Pos::new(4, 0), Pos::new(4, 12));
    assert_eq!(
        p.hints(text, Some(range)),
        "<?php\nnamespace App;\n\nfmt(1, 'a');\nfmt(⟨cents:⟩2, ⟨currency:⟩'b');\n$x = (new Library())->checkout('3');\n"
    );
}

/// Array shapes read `array` unless the settings ask for them in full; the
/// tooltip keeps the full type, and the other hints stay as they are.
#[test]
fn compact_array_shapes_in_type_hints() {
    let mut p = Project::new();
    let text = r#"<?php
namespace App;

final class Example
{
    private const REGISTER_FIELDS = ['id' => 1, 'name' => 2];
    private const QUERY_FIELDS = ['id' => 1, 'status' => 2];

    public function fields(bool $registering, Library $lib): array
    {
        $fields = $registering ? self::REGISTER_FIELDS : self::QUERY_FIELDS;
        $one = self::REGISTER_FIELDS;
        $mixed = $registering ? self::REGISTER_FIELDS : 'none';
        $maybe = $registering ? self::REGISTER_FIELDS : null;
        $flag = $registering;
        $lent = $lib->checkout('1');
        return $fields;
    }
}
"#;
    let full = p.hints_with(text, None, InlayHintSettings { array_shapes: ArrayShapeHints::Full });
    assert!(full.contains("$fields⟨: array{id: int, name: int}|array{id: int, status: int}⟩ ="), "{full}");
    assert!(full.contains("$one⟨: array{id: int, name: int}⟩ ="), "{full}");

    // Compact unless the settings say otherwise.
    let compact = InlayHintSettings::default();
    assert_eq!(compact.array_shapes, ArrayShapeHints::Compact);
    let shown = p.hints(text, None);
    assert!(shown.contains("$fields⟨: array⟩ ="), "{shown}");
    assert!(shown.contains("$one⟨: array⟩ ="), "{shown}");
    assert!(shown.contains("$mixed⟨: array|string⟩ ="), "{shown}");
    assert!(shown.contains("$maybe⟨: ?array⟩ ="), "{shown}");
    // Nothing else changes.
    assert!(shown.contains("$flag⟨: bool⟩ ="), "{shown}");
    assert!(shown.contains("$lent⟨: Book⟩ ="), "{shown}");
    for line in ["$flag", "$lent"] {
        let of = |hints: &str| hints.lines().find(|l| l.contains(line)).map(str::to_owned);
        assert_eq!(of(&full), of(&shown));
    }

    // The tooltip has the type in full.
    let infos = p.hint_infos(text, None, compact);
    let fields = infos.iter().find(|hint| hint.label == ": array").expect("the hint of $fields");
    assert_eq!(fields.tooltip.as_deref(), Some("array{id: int, name: int}|array{id: int, status: int}"));
}

#[test]
fn compact_shapes_inside_other_types() {
    let compact =
        |doc: &str| super::inlay::compact_shapes(&crate::types::parse_doc_type(doc, &[]).unwrap()).to_string();
    assert_eq!(compact("array{id: int}|array{name: string}|null"), "?array");
    assert_eq!(compact("list<array{id: int}>"), "list<array>");
    assert_eq!(compact("list{int, string}"), "list");
    // Only shapes: other array types say what they hold in a few words.
    assert_eq!(compact("array<string, int>"), "array<string, int>");
    assert_eq!(compact("list<int>|false"), "list<int>|false");
}

#[test]
fn short_types() {
    let ty = Type::union(vec![Type::named(r"App\Model\Book"), Type::Null]);
    assert_eq!(super::inlay::short_type(&ty), "?Book");
}
