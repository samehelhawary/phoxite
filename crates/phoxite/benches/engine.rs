//! `cargo bench -p phoxite`: how long the engine takes for what its
//! performance budgets are about (criterion benches). The budgets of the
//! first five are in `shared/workload.rs`, and `tests/budgets.rs` fails
//! when one is exceeded:
//!
//! - cold index (no cache) and warm start (cache from an earlier run): open
//!   the engine and wait until it is ready,
//! - hover and completion right after a keystroke (the parse and the type
//!   cache are both stale), under 50 ms,
//! - diagnostics after an edit: from the change until the background run
//!   has inspected the new version (debounce included), under 300 ms.
//!
//! `php_fixture/*` runs on `fixtures/php-library`. `php_laravel/*` runs on
//! laravel/framework from the corpora (`PHP_CORPUS_DIR`, default
//! `../php-corpus` next to the workspace; `scripts/php-corpus.sh`) and is
//! skipped when the corpus isn't there: cold index under 30 s, warm start
//! under 2 s, and the request latencies on `Eloquent/Builder.php`
//! (2.5k lines).
//!
//! The Laravel support budgets: `php_laravel_app/*` on
//! `fixtures/laravel-app` (with `fixtures/laravel-stubs` as its `vendor/`),
//! and `php_laravel_real/*` on a real application with `vendor/` when
//! `LARAVEL_APP_DIR` points to one (skipped otherwise):
//!
//! - `model_cold`: the whole project model (`PhpEngine::laravel_model`)
//!   after `PhpEngine::forget_laravel_model`, so `composer.json`, `ide.json`
//!   and the module manifests are read again and no part is stored, under
//!   2 s (400 ms on the real application, phase 9.4),
//! - `model_warm_start`: the same in a new session on the cache of an
//!   earlier one, which stored the parts read from single files (phase 9.4),
//!   under 150 ms on the real application,
//! - `model_warm`: the same right after a keystroke in a route file, under
//!   200 ms,
//! - `string_completion`: completion in `route('…')` right after a
//!   keystroke, under 50 ms,
//! - `route_hints`: the route hints of the whole route file right after a
//!   keystroke, under 100 ms,
//! - `inspect_code`: batch Inspect Code (`PhpEngine::inspect_files`, in
//!   parallel since phase 9.4) over the application's own PHP files and
//!   Blade templates (not `vendor/`), every built-in inspection on, under
//!   20 s on the real application.
//!
//! These run with no inspections, so nothing builds the model in the
//! background: each timed request pays for what it rebuilds.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use criterion::{Criterion, SamplingMode, black_box, criterion_group, criterion_main};
use phoxite::base::{CancellationToken, DocId, Pos, Range};
use phoxite::engine::PhpEngine;
use phoxite::inspect::Inspections;

/// The workloads and their timed operations, shared with the budget test
/// (which reads the budgets).
#[allow(dead_code)]
#[path = "shared/workload.rs"]
mod workload;

use workload::{WAIT, Workload, canonical, fixture, laravel, open_engine, pos_of};

fn bench(c: &mut Criterion) {
    let workloads: Vec<Workload> = std::iter::once(fixture()).chain(laravel()).collect();
    for workload in &workloads {
        bench_workload(c, workload);
    }
    for app in laravel_apps() {
        bench_laravel_app(c, &app);
    }
}

/// A Laravel application for the Laravel support benches.
struct LaravelApp {
    group: &'static str,
    root: PathBuf,
    /// The temporary copy `root` is in, for the fixture.
    _copy: Option<tempfile::TempDir>,
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The fixture app (copied with its stub `vendor/`), and the application of
/// `LARAVEL_APP_DIR` when it is set.
fn laravel_apps() -> Vec<LaravelApp> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let copy = tempfile::tempdir().unwrap();
    let root = canonical(copy.path()).join("laravel-app");
    copy_dir(&fixtures.join("laravel-app"), &root);
    copy_dir(&fixtures.join("laravel-stubs"), &root.join("vendor"));
    let mut out = vec![LaravelApp { group: "php_laravel_app", root, _copy: Some(copy) }];
    match std::env::var_os("LARAVEL_APP_DIR").map(PathBuf::from) {
        Some(dir) if dir.join("artisan").is_file() && dir.join("vendor").is_dir() => {
            out.push(LaravelApp { group: "php_laravel_real", root: canonical(&dir), _copy: None });
        }
        Some(dir) => eprintln!("php_laravel_real: {} has no artisan or vendor/, skipped", dir.display()),
        None => eprintln!("php_laravel_real: LARAVEL_APP_DIR isn't set, skipped"),
    }
    out
}

/// `routes/web.php` when it registers routes, else the file registering
/// the most.
fn route_file(engine: &PhpEngine, root: &Path) -> PathBuf {
    let routes = engine.laravel_routes().expect("the routes");
    let mut counts: Vec<(PathBuf, usize)> = Vec::new();
    for route in routes.routes() {
        match counts.iter_mut().find(|(path, _)| *path == route.path) {
            Some((_, count)) => *count += 1,
            None => counts.push((route.path.clone(), 1)),
        }
    }
    let web = root.join("routes").join("web.php");
    let is_web = |path: &Path| path.strip_prefix(root).is_ok_and(|rel| rel == Path::new("routes").join("web.php"));
    if let Some((path, _)) = counts.iter().find(|(path, _)| is_web(path)) {
        return path.clone();
    }
    counts.into_iter().max_by_key(|(_, count)| *count).map_or(web, |(path, _)| path)
}

fn bench_laravel_app(c: &mut Criterion, app: &LaravelApp) {
    let mut g = c.benchmark_group(app.group);
    g.sample_size(20).sampling_mode(SamplingMode::Auto).warm_up_time(Duration::from_millis(500));
    g.measurement_time(Duration::from_secs(5));
    let cache = tempfile::tempdir().unwrap();
    let engine = open_engine(&app.root, cache.path());
    engine.set_inspections(Inspections::default());
    let summary = engine.laravel_model().unwrap_or_else(|| panic!("{}: not a Laravel project", app.root.display()));
    eprintln!("{}: {summary:?}", app.group);

    // The route file open in the editor: `routes/web.php` when it has
    // routes, else the file with the most (an app whose routes come from
    // packages). A keystroke adds a statement at its end, a different one
    // each time.
    let path = route_file(&engine, &app.root);
    eprintln!("{}: route file {}", app.group, path.display());
    let original = std::fs::read_to_string(&path).expect("the route file");
    let doc = DocId(1);
    let mut version = 1;
    engine.document_opened(doc, &path, version, &original);
    assert!(engine.wait_inspections(WAIT));
    let edited = |n: u32| format!("{original}\n$bench{n} = {n};\n");

    g.bench_function("model_cold", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                engine.forget_laravel_model();
                let started = Instant::now();
                let built = engine.laravel_model();
                total += started.elapsed();
                assert!(black_box(built).is_some());
            }
            total
        })
    });

    g.bench_function("model_warm", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                version += 1;
                engine.document_changed(doc, version, &edited(version));
                assert!(engine.wait_inspections(WAIT));
                let started = Instant::now();
                let built = engine.laravel_model();
                total += started.elapsed();
                assert!(black_box(built).is_some());
            }
            total
        })
    });

    // `route('` typed on a new line after the keystroke's statement.
    g.bench_function("string_completion", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                version += 1;
                let text = format!("{}route('');\n", edited(version));
                let at = pos_of(&text, text.len() - "');\n".len());
                engine.document_changed(doc, version, &text);
                assert!(engine.wait_inspections(WAIT));
                let started = Instant::now();
                let found = engine.completion(doc, at, Some("'"), &CancellationToken::new());
                total += started.elapsed();
                assert!(found.is_some_and(|found| !found.items.is_empty()), "{}: no route names", app.group);
            }
            total
        })
    });

    g.bench_function("route_hints", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                version += 1;
                let text = edited(version);
                let end = pos_of(&text, text.len());
                engine.document_changed(doc, version, &text);
                assert!(engine.wait_inspections(WAIT));
                let started = Instant::now();
                let hints = engine.laravel_route_hints(doc, Range { start: Pos::default(), end });
                total += started.elapsed();
                assert!(black_box(hints).is_some_and(|hints| !hints.is_empty()), "{}: no route hints", app.group);
            }
            total
        })
    });

    // Inspect Code over the application's own code, every built-in
    // inspection on. Seconds per run on a real application: single runs.
    let files = own_code(&app.root);
    eprintln!("{}: Inspect Code over {} files", app.group, files.len());
    engine.set_inspections(Inspections::builtin());
    g.sample_size(10).sampling_mode(SamplingMode::Flat).warm_up_time(Duration::from_millis(1));
    g.measurement_time(Duration::from_secs(1));
    g.bench_function("inspect_code", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let problems = std::sync::atomic::AtomicUsize::new(0);
                let started = Instant::now();
                let done = engine.inspect_files(
                    &files,
                    |path| path.as_path(),
                    |path| std::fs::read_to_string(path).ok(),
                    &CancellationToken::new(),
                    |_, found| {
                        let found = found.map_or(0, |found| found.len());
                        problems.fetch_add(found, std::sync::atomic::Ordering::Relaxed);
                    },
                );
                total += started.elapsed();
                assert!(done);
                black_box(problems.into_inner());
            }
            total
        })
    });
    engine.set_inspections(Inspections::default());

    // A new session on this cache: the parts read from single files are
    // stored (written by the worker after the builds above), so the build
    // takes them from the index database. Each run opens the engine (not
    // timed) and builds the model.
    assert!(engine.laravel_model().is_some());
    assert!(engine.wait_idle(WAIT));
    assert!(engine.laravel_stored_parts() > 0, "{}: no part stored", app.group);
    engine.shutdown();
    drop(engine);
    g.bench_function("model_warm_start", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let engine = open_engine(&app.root, cache.path());
                engine.set_inspections(Inspections::default());
                let started = Instant::now();
                let built = engine.laravel_model();
                total += started.elapsed();
                assert!(black_box(built).is_some());
                assert!(!engine.laravel_part_loads().is_empty(), "{}: nothing taken from the store", app.group);
                engine.shutdown();
            }
            total
        })
    });

    g.finish();
}

/// The application's own PHP files and Blade templates: `app/`,
/// `packages/`, `database/`, `routes/` and the templates of `resources/`,
/// never `vendor/` or `node_modules/`.
fn own_code(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, php: bool, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if path.is_dir() {
                if name != "vendor" && name != "node_modules" && !name.starts_with('.') {
                    walk(&path, php, out);
                }
            } else if name.ends_with(".blade.php") || (php && name.ends_with(".php")) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for dir in ["app", "packages", "database", "routes"] {
        walk(&root.join(dir), true, &mut out);
    }
    walk(&root.join("resources"), false, &mut out);
    out
}

fn bench_workload(c: &mut Criterion, w: &Workload) {
    let mut g = c.benchmark_group(w.group);

    // Cold index: an empty cache every time. Laravel takes seconds per run,
    // so the samples are single runs and there is no warm-up worth the name.
    g.sample_size(10).sampling_mode(SamplingMode::Flat).warm_up_time(Duration::from_millis(1));
    g.measurement_time(Duration::from_secs(1));
    g.bench_function("cold_index", |b| b.iter_custom(|iters| (0..iters).map(|_| w.cold_index()).sum()));

    let cache = w.warm_cache();
    g.bench_function("warm_start", |b| b.iter_custom(|iters| (0..iters).map(|_| w.warm_start(cache.path())).sum()));

    // The requests: one engine with the file open, as in the editor.
    g.sample_size(30).sampling_mode(SamplingMode::Auto).warm_up_time(Duration::from_millis(500));
    g.measurement_time(Duration::from_secs(5));
    let mut session = w.session();
    g.bench_function("hover_after_keystroke", |b| {
        b.iter_custom(|iters| (0..iters).map(|_| session.hover_after_keystroke()).sum())
    });
    g.bench_function("completion_after_keystroke", |b| {
        b.iter_custom(|iters| (0..iters).map(|_| session.completion_after_keystroke()).sum())
    });
    g.bench_function("diagnostics_after_edit", |b| {
        b.iter_custom(|iters| (0..iters).map(|_| session.diagnostics_after_edit()).sum())
    });

    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
