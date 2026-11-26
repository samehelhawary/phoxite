//! The performance budgets, as a test that fails when one is exceeded.
//! Ignored: the timings mean something in a release build only.
//!
//! ```text
//! cargo test -p phoxite --release --test budgets -- --ignored --nocapture
//! ```
//!
//! It times what `benches/engine.rs` times on `fixtures/php-library` and,
//! when the corpora are there (`scripts/php-corpus.sh`, `PHP_CORPUS_DIR`),
//! on laravel/framework: cold index, warm start, hover and completion
//! right after a keystroke, and diagnostics after an edit. Each is run
//! several times; the median has to be within its budget
//! (`benches/shared/workload.rs`) times `PHOXITE_BUDGET_FACTOR` (1 unless
//! set; CI's shared machines use 3).
//!
//! The medians are written to `target/php-budgets.json`, which
//! `scripts/bench_report.mjs` puts in the README. A debug build writes them
//! too and fails nothing.

#[allow(dead_code)]
#[path = "../benches/shared/workload.rs"]
mod workload;

use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use workload::{Workload, fixture, laravel, magento};

/// Runs of the operations that take the longest (opening the engine).
const STARTUP_RUNS: usize = 5;
/// Runs of each request, after [`WARM_UP_RUNS`] that aren't counted.
const REQUEST_RUNS: usize = 15;
const WARM_UP_RUNS: usize = 3;

fn median(mut runs: Vec<Duration>) -> Duration {
    runs.sort();
    runs[runs.len() / 2]
}

fn timed(runs: usize, mut run: impl FnMut() -> Duration) -> Duration {
    median((0..runs).map(|_| run()).collect())
}

/// The median of each timed operation of `w` with its budget, and the
/// number of files indexed.
fn measure(w: &Workload) -> (u64, Vec<(&'static str, Duration, Duration)>) {
    let cold = timed(STARTUP_RUNS, || w.cold_index());
    let cache = w.warm_cache();
    let warm = timed(STARTUP_RUNS, || w.warm_start(cache.path()));

    let mut session = w.session();
    let files = session.engine.status().files;
    let mut request = |run: &mut dyn FnMut(&mut workload::Session) -> Duration| {
        for _ in 0..WARM_UP_RUNS {
            run(&mut session);
        }
        timed(REQUEST_RUNS, || run(&mut session))
    };
    let hover = request(&mut |s| s.hover_after_keystroke());
    let completion = request(&mut |s| s.completion_after_keystroke());
    let diagnostics = request(&mut |s| s.diagnostics_after_edit());

    let b = &w.budgets;
    let measured = vec![
        ("cold_index", cold, b.cold_index),
        ("warm_start", warm, b.warm_start),
        ("hover_after_keystroke", hover, b.hover_after_keystroke),
        ("completion_after_keystroke", completion, b.completion_after_keystroke),
        ("diagnostics_after_edit", diagnostics, b.diagnostics_after_edit),
    ];
    (files, measured)
}

fn millis(d: Duration) -> f64 {
    (d.as_secs_f64() * 1e5).round() / 100.0
}

#[test]
#[ignore = "timings: run it in a release build"]
fn the_engine_is_within_its_budgets() {
    let factor: f64 = match std::env::var("PHOXITE_BUDGET_FACTOR") {
        Ok(text) => text.parse().unwrap_or_else(|_| panic!("PHOXITE_BUDGET_FACTOR={text} is no number")),
        Err(_) => 1.0,
    };
    let release = !cfg!(debug_assertions);
    let workloads: Vec<Workload> = std::iter::once(fixture()).chain(laravel()).chain(magento()).collect();

    let mut results = Vec::new();
    let mut exceeded = Vec::new();
    for w in &workloads {
        let (files, measured) = measure(w);
        for (name, median, budget) in measured {
            let limit = budget.mul_f64(factor);
            let ok = median <= limit;
            println!(
                "{:<12} {name:<27} {:>10.2} ms   budget {:>6.0} ms{}",
                w.group,
                millis(median),
                millis(limit),
                if ok { "" } else { "   EXCEEDED" }
            );
            if !ok {
                exceeded.push(format!("{}/{name}: {:.2} ms, budget {:.0} ms", w.group, millis(median), millis(limit)));
            }
            results.push(json!({
                "workload": w.group,
                "measure": name,
                "files": files,
                "median_ms": millis(median),
                "budget_ms": millis(budget),
                "within_budget": ok,
            }));
        }
    }

    let unix_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let report: Value = json!({
        "unix_time": unix_time,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "threads": std::thread::available_parallelism().map_or(0, |n| n.get()),
        "release": release,
        "budget_factor": factor,
        "results": results,
    });
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/php-budgets.json");
    std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap() + "\n").unwrap();
    println!("report: {}", out.display());

    if !release {
        println!("a debug build: the budgets are for a release build, nothing is checked");
        return;
    }
    assert!(exceeded.is_empty(), "over budget:\n{}", exceeded.join("\n"));
}
