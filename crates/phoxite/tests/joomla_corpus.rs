//! Joomla support on Joomla itself (Joomla 6.2.0, the corpus `joomla`), with the framework
//! packages it installs with Composer as include paths (registry, DI, event, database and
//! input: the repository has no `libraries/vendor`).
//!
//! The engine's own tests use a small stand-in; this one checks that what they assume holds
//! for the real source: that the extensions, language files, layouts, assets, forms, events
//! and tables are read, that the names resolve, that the application, a component's
//! classes, a view's template and an event handler's parameter are typed, and what Joomla's
//! inspections say about Joomla's own code.
//!
//! Ignored: it needs the corpora (`scripts/php-corpus.sh joomla joomla-registry joomla-di
//! joomla-event joomla-database joomla-input`).
//!
//! ```text
//! cargo test -p phoxite --release --test joomla_corpus -- --ignored --nocapture
//! ```
//!
//! Environment: `PHP_CORPUS_DIR` (default `../php-corpus` next to the workspace).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, PhpEngine};
use phoxite::frameworks::Framework;
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;

const CARET: &str = "/*|*/";

fn corpus(name: &str) -> PathBuf {
    let root = std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"))
        .join(name);
    dunce::canonicalize(root).expect("the corpus (scripts/php-corpus.sh)")
}

struct Site {
    engine: Arc<PhpEngine>,
    root: PathBuf,
    next: u32,
    _cache: tempfile::TempDir,
}

fn php_files(folder: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            php_files(&path, out);
        } else if phoxite::project::is_php_path(&path) {
            out.push(path);
        }
    }
}

impl Site {
    fn open() -> Self {
        let root = corpus("joomla");
        let cache = tempfile::tempdir().unwrap();
        let packages = ["joomla-registry", "joomla-di", "joomla-event", "joomla-database", "joomla-input"];
        let include: Vec<PathBuf> = packages.iter().map(|package| corpus(package).join("src")).collect();
        let config = EngineConfig {
            cache_dir: cache.path().join(CACHE_SUBDIR),
            layout: LayoutConfig { root: root.clone(), include_paths: include, ..LayoutConfig::default() },
            stubs: StubFilter::default(),
        };
        let sink: SharedSink = Arc::new(|_: IdeEvent| {});
        let started = Instant::now();
        let engine = PhpEngine::open(config, sink).unwrap();
        assert!(engine.wait_idle(Duration::from_secs(1800)), "the corpus indexes");
        println!("indexed in {:?}", started.elapsed());
        Self { engine, root, next: 1, _cache: cache }
    }

    fn document(&mut self, file: &str, text: &str) -> DocId {
        let doc = DocId(self.next);
        self.next += 1;
        self.engine.document_opened(doc, &self.root.join(file), 1, text);
        doc
    }

    fn at(&mut self, file: &str, from: &str, to: &str) -> (DocId, Pos) {
        let source = std::fs::read_to_string(self.root.join(file)).unwrap().replace("\r\n", "\n");
        assert!(source.contains(from), "{from} in {file}");
        let source = source.replacen(from, to, 1);
        let at = source.find(CARET).expect("a caret");
        let text = format!("{}{}", &source[..at], &source[at + CARET.len()..]);
        let line = text[..at].matches('\n').count() as u32;
        let col = text[..at].rsplit('\n').next().unwrap_or_default().encode_utf16().count() as u32;
        (self.document(file, &text), Pos::new(line, col))
    }

    fn goes_to(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.definition(doc, pos).unwrap_or_default();
        self.engine.document_closed(doc);
        found
            .iter()
            .map(|target| {
                let path = target.path.strip_prefix(&self.root).unwrap_or(&target.path);
                path.to_string_lossy().replace('\\', "/")
            })
            .collect()
    }

    fn offered(&mut self, file: &str, from: &str, to: &str) -> Vec<String> {
        let (doc, pos) = self.at(file, from, to);
        let found = self.engine.completion(doc, pos, None, &CancellationToken::new()).expect("handled");
        self.engine.document_closed(doc);
        found.items.into_iter().map(|item| item.label).collect()
    }

    /// The reports of Joomla's inspections on the PHP files of `folders`, the files
    /// inspected, and how many reports the other inspections make.
    fn reports(&mut self, folders: &[&str]) -> (Vec<String>, usize, usize) {
        let mut files = Vec::new();
        for folder in folders {
            php_files(&self.root.join(folder), &mut files);
        }
        files.sort();
        let (mut unknown, mut others) = (Vec::new(), 0);
        let count = files.len();
        for path in files {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let file = path.strip_prefix(&self.root).unwrap().to_string_lossy().replace('\\', "/");
            let doc = self.document(&file, &text);
            let found = self.engine.inspect(doc, &CancellationToken::new()).expect("open");
            for problem in &found.problems {
                if problem.inspection.contains("Joomla") {
                    let line = problem.range.start.line + 1;
                    unknown.push(format!("{file}:{line} {} {}", problem.inspection, problem.message));
                } else {
                    others += 1;
                }
            }
            self.engine.document_closed(doc);
        }
        (unknown, count, others)
    }
}

fn has(found: &[String], names: &[&str]) {
    for name in names {
        assert!(found.contains(&(*name).to_owned()), "{name}: {found:?}");
    }
}

#[test]
#[ignore = "needs the joomla corpus"]
fn joomla_itself() {
    let mut site = Site::open();
    assert!(site.engine.frameworks().contains(Framework::Joomla));

    // The model: read once, kept, read again when one of its files is written.
    let started = Instant::now();
    let project = site.engine.joomla();
    let cold = started.elapsed();
    let started = Instant::now();
    assert!(Arc::ptr_eq(&project, &site.engine.joomla()));
    let kept = started.elapsed();
    let touched = site.root.join("language/en-GB/com_content.ini");
    site.engine.files_changed(std::slice::from_ref(&touched), &[]);
    let started = Instant::now();
    let reread = site.engine.joomla();
    println!("model: {cold:?} cold, {kept:?} kept, {:?} after a language file changed", started.elapsed());
    assert_eq!(reread.constants.len(), project.constants.len());

    println!(
        "{} extension folders, {} language constants, {} layouts, {} assets, {} ACL actions, {} forms, {} events, {} tables, {} files read",
        project.extensions.len(),
        project.constants.len(),
        project.layouts.len(),
        project.assets.len(),
        project.actions.len(),
        project.forms.len(),
        project.events.len(),
        project.tables.len(),
        project.files.len(),
    );
    assert!(project.extensions.len() > 250 && project.constants.len() > 8000, "{}", project.extensions.len());
    assert!(project.layouts.len() > 150 && project.assets.len() > 200 && project.actions.len() > 100);
    assert!(project.forms.len() > 150 && project.events.len() > 100 && project.tables.len() > 60);

    // Some of it, looked up.
    let content = project.extension("com_content");
    let namespaces: Vec<&str> = content.iter().filter_map(|folder| folder.namespace.as_deref()).collect();
    has(
        &namespaces.iter().map(|namespace| (*namespace).to_owned()).collect::<Vec<_>>(),
        &[
            "Joomla\\Component\\Content\\Site",
            "Joomla\\Component\\Content\\Administrator",
            "Joomla\\Component\\Content\\Api",
        ],
    );
    let pagebreak = project.extension("plg_content_pagebreak");
    assert_eq!(pagebreak[0].namespace.as_deref(), Some("Joomla\\Plugin\\Content\\PageBreak"));
    assert_eq!(project.constant("COM_CONTENT_ARTICLE_HITS")[0].detail.as_deref(), Some("Hits: %d"));
    assert!(!project.layout("joomla.content.tags").is_empty());
    assert_eq!(project.event_class("onContentPrepare"), Some("Joomla\\CMS\\Event\\Content\\ContentPrepareEvent"));
    assert!(project.table("#__content").is_some_and(|table| table.columns.iter().any(|column| column.0 == "title")));
    assert!(project.assets.iter().any(|asset| asset.name == "com_content.admin-article-pagebreak"));
    assert!(project.actions.iter().any(|action| action.name == "core.edit.state"));
    assert!(
        project.forms.iter().any(|form| form.name == "article" && form.fields.iter().any(|field| field.0 == "title"))
    );

    // Names in core code: a subscriber's event and its handler, a constant, a layout.
    let plugin = "plugins/content/pagebreak/src/Extension/PageBreak.php";
    let event = site.goes_to(
        plugin,
        "'onContentPrepare' => 'onContentPrepare'",
        "'onContent/*|*/Prepare' => 'onContentPrepare'",
    );
    assert_eq!(event, ["libraries/src/Event/Content/ContentPrepareEvent.php"]);
    let handler = site.goes_to(
        plugin,
        "'onContentPrepare' => 'onContentPrepare'",
        "'onContentPrepare' => 'onContent/*|*/Prepare'",
    );
    assert_eq!(handler, [plugin]);
    let constant = site.goes_to(
        plugin,
        "sprintf('PLG_CONTENT_PAGEBREAK_PAGE_NUM'",
        "sprintf('PLG_CONTENT_PAGEBREAK/*|*/_PAGE_NUM'",
    );
    assert!(constant.iter().any(|file| file.ends_with("plg_content_pagebreak.ini")), "{constant:?}");
    // (Through the language object too.)
    let constant = site.goes_to(
        plugin,
        "_('PLG_CONTENT_PAGEBREAK_ARTICLE_INDEX')",
        "_('PLG_CONTENT_PAGEBREAK/*|*/_ARTICLE_INDEX')",
    );
    assert!(constant.iter().any(|file| file.ends_with("plg_content_pagebreak.ini")), "{constant:?}");

    // Types: a view's template, the application, an event handler's parameter.
    let template = "components/com_content/tmpl/article/default.php";
    let offered = site.offered(template, "<?php", "<?php $this->/*|*/; ?>\n<?php");
    has(&offered, &["item", "params", "loadTemplate", "escape"]);
    let offered = site.offered(
        plugin,
        "$context = $event->getContext();",
        "$event->/*|*/;\n        $context = $event->getContext();",
    );
    has(&offered, &["getContext", "getItem", "getParams"]);
    let offered = site.offered(
        plugin,
        "$context = $event->getContext();",
        "\\Joomla\\CMS\\Factory::getApplication()->/*|*/;\n        $context = $event->getContext();",
    );
    has(&offered, &["getDocument", "getIdentity", "bootComponent", "getInput"]);

    // Joomla's inspections on a sample of core code.
    let folders =
        ["components/com_content", "administrator/components/com_content", "plugins/content", "modules/mod_custom"];
    let started = Instant::now();
    let (unknown, files, others) = site.reports(&folders);
    println!("{files} files inspected in {:?}: {others} reports of other inspections", started.elapsed());
    println!("{}", unknown.join("\n"));
    // One, and it is right: Joomla 6.2 renders a layout it doesn't have.
    assert_eq!(unknown.len(), 1, "{unknown:?}");
    assert!(unknown[0].contains("Unknown layout 'joomla.content.icons.print_screen'"), "{unknown:?}");
    assert!(!site.root.join("layouts/joomla/content/icons/print_screen.php").is_file());
}
