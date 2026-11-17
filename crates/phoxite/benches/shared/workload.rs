//! The projects the request benches (`benches/engine.rs`) and the budget
//! test (`tests/budgets.rs`) time, each operation they time, and the budget
//! of each: the benches report how long the operations take, the test fails
//! when one takes longer than its budget.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, EngineState, PhpEngine};
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

/// Indexing laravel takes seconds; anything over this is a hang.
pub const WAIT: Duration = Duration::from_secs(600);

/// The longest each timed operation of a workload may take.
pub struct Budgets {
    /// Empty cache → ready.
    pub cold_index: Duration,
    /// The cache of an earlier run → ready.
    pub warm_start: Duration,
    pub hover_after_keystroke: Duration,
    pub completion_after_keystroke: Duration,
    /// From the edit until the background run has inspected the new
    /// version, debounce included.
    pub diagnostics_after_edit: Duration,
}

/// A project and the edit, hover and completion the request benches make
/// in one of its files.
pub struct Workload {
    pub group: &'static str,
    pub root: PathBuf,
    /// The file opened in the editor, relative to `root`.
    pub file: &'static str,
    /// Hover on the first occurrence of `.0`, `.1` bytes into it.
    pub hover: (&'static str, usize),
    /// The statement after which a line is typed.
    pub anchor: &'static str,
    /// The line typed for completion, which completes at its end.
    pub typed: &'static str,
    pub budgets: Budgets,
}

const fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

pub fn fixture() -> Workload {
    Workload {
        group: "php_fixture",
        root: canonical(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/php-library")),
        file: "src/Library.php",
        hover: ("->save($book)", 2),
        anchor: "$lent = $book->withAvailability(false);",
        typed: "        $lent->",
        budgets: Budgets {
            cold_index: ms(500),
            warm_start: ms(200),
            hover_after_keystroke: ms(50),
            completion_after_keystroke: ms(50),
            diagnostics_after_edit: ms(300),
        },
    }
}

/// laravel/framework from the corpora, when it is there.
pub fn laravel() -> Option<Workload> {
    let base = std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"));
    let root = base.join("laravel-framework");
    if !root.join("composer.json").is_file() {
        eprintln!("php_laravel: no corpus at {} (scripts/php-corpus.sh), skipped", root.display());
        return None;
    }
    Some(Workload {
        group: "php_laravel",
        root: canonical(&root),
        file: "src/Illuminate/Database/Eloquent/Builder.php",
        hover: ("$this->query->whereIn(", 14),
        anchor: "$this->query->whereIn($this->model->getQualifiedKeyName(), $id);",
        typed: "                $this->query->",
        budgets: Budgets {
            cold_index: ms(30_000),
            warm_start: ms(2_000),
            hover_after_keystroke: ms(50),
            completion_after_keystroke: ms(50),
            diagnostics_after_edit: ms(300),
        },
    })
}

pub fn canonical(path: &Path) -> PathBuf {
    dunce::canonicalize(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Opens the engine on `root` with its cache in `cache` and waits until the
/// indexes are up to date.
pub fn open_engine(root: &Path, cache: &Path) -> Arc<PhpEngine> {
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let config = EngineConfig {
        cache_dir: cache.join(CACHE_SUBDIR),
        layout: LayoutConfig { root: root.to_path_buf(), ..LayoutConfig::default() },
        stubs: StubFilter::default(),
    };
    let engine = PhpEngine::open(config, sink).expect("engine opens");
    assert!(engine.wait_idle(WAIT), "indexing {} timed out", root.display());
    assert_eq!(engine.status().state, EngineState::Ready, "{}", root.display());
    engine
}

/// The 0-based line and UTF-16 column of byte `offset`.
pub fn pos_of(text: &str, offset: usize) -> Pos {
    let before = &text[..offset];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    Pos { line: before.matches('\n').count() as u32, col: before[line_start..].encode_utf16().count() as u32 }
}

impl Workload {
    /// Cold index: an empty cache, until the engine is ready.
    pub fn cold_index(&self) -> Duration {
        let cache = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let engine = open_engine(&self.root, cache.path());
        let took = started.elapsed();
        engine.shutdown();
        took
    }

    /// The cache an earlier run left, for [`warm_start`](Self::warm_start).
    pub fn warm_cache(&self) -> tempfile::TempDir {
        let cache = tempfile::tempdir().unwrap();
        open_engine(&self.root, cache.path()).shutdown();
        cache
    }

    /// Warm start: the cache of an earlier run, nothing changed on disk.
    pub fn warm_start(&self, cache: &Path) -> Duration {
        let started = Instant::now();
        let engine = open_engine(&self.root, cache);
        let took = started.elapsed();
        engine.shutdown();
        took
    }

    /// One engine with the file open, as in the editor.
    pub fn session(&self) -> Session {
        let cache = tempfile::tempdir().unwrap();
        let engine = open_engine(&self.root, cache.path());
        let path = self.root.join(self.file);
        let original = std::fs::read_to_string(&path).unwrap();
        let doc = DocId(1);
        engine.document_opened(doc, &path, 1, &original);
        assert!(engine.wait_inspections(WAIT));

        let anchor = original.find(self.anchor).unwrap_or_else(|| panic!("{} in {}", self.anchor, self.file))
            + self.anchor.len();
        let hover_at = pos_of(&original, original.find(self.hover.0).expect("hover target") + self.hover.1);
        assert!(engine.hover(doc, hover_at).is_some(), "{}: no hover at {hover_at:?}", self.group);
        let typed = format!("{}\n{}{}", &original[..anchor], self.typed, &original[anchor..]);
        let complete_at = pos_of(&typed, anchor + 1 + self.typed.len());
        Session {
            engine,
            doc,
            version: 1,
            indent: self.typed.chars().take_while(|c| *c == ' ').collect(),
            original,
            anchor,
            hover_at,
            typed,
            complete_at,
            _cache: cache,
        }
    }
}

/// An engine with a workload's file open, and the requests timed on it.
pub struct Session {
    pub engine: Arc<PhpEngine>,
    doc: DocId,
    version: u32,
    original: String,
    /// The end of the statement a line is typed after.
    anchor: usize,
    indent: String,
    hover_at: Pos,
    /// The file with the line typed for completion.
    typed: String,
    complete_at: Pos,
    _cache: tempfile::TempDir,
}

impl Session {
    /// A keystroke that changes nothing the requests look at: a statement
    /// after the anchor, a different one each time.
    fn edited(&mut self) -> String {
        self.version += 1;
        let n = self.version;
        let (before, after) = self.original.split_at(self.anchor);
        format!("{before}\n{}$bench{n} = {n};{after}", self.indent)
    }

    /// Hover right after a keystroke: the parse and the type cache are
    /// stale. The edit comes after the hover target, which stays put.
    pub fn hover_after_keystroke(&mut self) -> Duration {
        let text = self.edited();
        self.engine.document_changed(self.doc, self.version, &text);
        assert!(self.engine.wait_inspections(WAIT));
        let started = Instant::now();
        let found = self.engine.hover(self.doc, self.hover_at);
        let took = started.elapsed();
        assert!(found.is_some());
        took
    }

    pub fn completion_after_keystroke(&mut self) -> Duration {
        self.version += 1;
        self.engine.document_changed(self.doc, self.version, &self.typed);
        assert!(self.engine.wait_inspections(WAIT));
        let started = Instant::now();
        let found = self.engine.completion(self.doc, self.complete_at, Some(">"), &CancellationToken::new());
        let took = started.elapsed();
        assert!(found.is_some_and(|found| !found.items.is_empty()));
        took
    }

    /// From the edit until the background run published the new version's
    /// diagnostics: the debounce plus every built-in inspection (the
    /// default set).
    pub fn diagnostics_after_edit(&mut self) -> Duration {
        let text = self.edited();
        let started = Instant::now();
        self.engine.document_changed(self.doc, self.version, &text);
        assert!(self.engine.wait_inspections(WAIT));
        let took = started.elapsed();
        assert_eq!(self.engine.inspected(self.doc).expect("open").version, self.version);
        took
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.engine.shutdown();
    }
}
