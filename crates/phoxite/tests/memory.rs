//! Memory of the native engine against Intelephense on laravel/framework. Ignored: it needs the corpora
//! (`scripts/php-corpus.sh`, `PHP_CORPUS_DIR`) and Intelephense
//! (`npm i -g intelephense`).
//!
//! ```text
//! cargo test -p phoxite --release --test memory -- --ignored --nocapture
//! ```
//!
//! - **Engine:** the working set of this process before the engine opens,
//!   after a cold index (empty cache) and after hover, completion and the
//!   inspections on `Eloquent/Builder.php`, and the peak.
//! - **Intelephense:** started with `node` directly (so the measured process
//!   is the server, not a shell), a fresh storage directory, the corpus as
//!   its workspace; measured once it sends `indexingEnded`.
//!
//! Working sets are read from the OS: `Get-Process` on Windows, `/proc` on
//! Linux, `ps` on macOS (which has no peak to give). Elsewhere the test only
//! reports that it can't measure.
//!
//! The report goes to `target/php-memory.txt` and, for
//! `scripts/bench_report.mjs`, `target/php-memory.json`.

#[allow(dead_code)]
#[path = "support/lsp_client.rs"]
mod lsp_client;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use phoxite::base::{CancellationToken, DocId, IdeEvent, Pos, SharedSink};
use phoxite::engine::{CACHE_SUBDIR, EngineConfig, EngineState, PhpEngine};
use phoxite::project::LayoutConfig;
use phoxite::stubs::StubFilter;
use serde_json::json;

const INDEX_TIMEOUT: Duration = Duration::from_secs(900);
const MB: f64 = 1024.0 * 1024.0;

fn corpus() -> PathBuf {
    let base = std::env::var_os("PHP_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../php-corpus"));
    let root = base.join("laravel-framework");
    assert!(root.join("composer.json").is_file(), "no corpus at {} (run scripts/php-corpus.sh)", root.display());
    dunce::canonicalize(root).unwrap()
}

/// The working set and its peak (where the OS keeps one), in bytes.
#[derive(Clone, Copy, Debug)]
struct Memory {
    current: u64,
    peak: Option<u64>,
}

#[cfg(windows)]
fn memory(pid: u32) -> Option<Memory> {
    let script = format!("$p = Get-Process -Id {pid}; \"$($p.WorkingSet64) $($p.PeakWorkingSet64)\"");
    let out = Command::new("powershell").args(["-NoProfile", "-Command", &script]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace().map(|n| n.parse::<u64>().ok());
    Some(Memory { current: parts.next()??, peak: parts.next()? })
}

#[cfg(target_os = "linux")]
fn memory(pid: u32) -> Option<Memory> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let kb = |key: &str| -> Option<u64> {
        let line = status.lines().find(|line| line.starts_with(key))?;
        Some(line.split_whitespace().nth(1)?.parse::<u64>().ok()? * 1024)
    };
    Some(Memory { current: kb("VmRSS:")?, peak: kb("VmHWM:") })
}

#[cfg(target_os = "macos")]
fn memory(pid: u32) -> Option<Memory> {
    let out = Command::new("ps").args(["-o", "rss=", "-p", &pid.to_string()]).output().ok()?;
    let kb = String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok()?;
    Some(Memory { current: kb * 1024, peak: None })
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn memory(_pid: u32) -> Option<Memory> {
    None
}

fn mb(bytes: u64) -> String {
    format!("{:.0} MB", bytes as f64 / MB)
}

fn peak(memory: Memory) -> String {
    memory.peak.map_or_else(|| "unknown".to_owned(), mb)
}

/// Megabytes, for the JSON report.
fn megabytes(bytes: u64) -> u64 {
    (bytes as f64 / MB).round() as u64
}

#[test]
#[ignore = "needs the corpora (scripts/php-corpus.sh) and Intelephense (npm i -g intelephense)"]
fn engine_memory_against_intelephense() {
    let root = corpus();
    let Some(before) = memory(std::process::id()) else {
        eprintln!("memory: can't read process memory on this platform");
        return;
    };

    // ------------------------------------------------------------ engine
    let cache = tempfile::tempdir().unwrap();
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let config = EngineConfig {
        cache_dir: cache.path().join(CACHE_SUBDIR),
        layout: LayoutConfig { root: root.clone(), ..LayoutConfig::default() },
        stubs: StubFilter::default(),
    };
    let started = Instant::now();
    let engine = PhpEngine::open(config, sink).unwrap();
    assert!(engine.wait_idle(INDEX_TIMEOUT));
    assert_eq!(engine.status().state, EngineState::Ready);
    let indexed = started.elapsed();
    let ready = memory(std::process::id()).unwrap();

    let path = root.join("src/Illuminate/Database/Eloquent/Builder.php");
    let text = std::fs::read_to_string(&path).unwrap();
    let doc = DocId(1);
    engine.document_opened(doc, &path, 1, &text);
    let offset = text.find("$this->query->whereIn(").unwrap() + "$this->query->".len();
    let line_start = text[..offset].rfind('\n').map_or(0, |i| i + 1);
    let pos = Pos { line: text[..offset].matches('\n').count() as u32, col: (offset - line_start) as u32 };
    assert!(engine.hover(doc, pos).is_some());
    assert!(engine.completion(doc, pos, None, &CancellationToken::new()).is_some());
    assert!(engine.inspect(doc, &CancellationToken::new()).is_some());
    assert!(engine.wait_inspections(INDEX_TIMEOUT));
    let working = memory(std::process::id()).unwrap();
    let files = engine.status().files;
    engine.shutdown();

    // ------------------------------------------------------ intelephense
    let storage = tempfile::tempdir().unwrap();
    let (_, intelephense_version) = lsp_client::intelephense_script();
    let (server, intelephense_indexed) = lsp_client::intelephense(&root, storage.path(), INDEX_TIMEOUT);
    // Let the post-indexing work settle before measuring.
    std::thread::sleep(Duration::from_secs(2));
    let intelephense = memory(server.pid()).expect("Intelephense's memory");
    server.shutdown(Duration::from_secs(10));

    let report = format!(
        "laravel/framework ({files} files)\n\
         engine:       cold index {:.1} s; working set {} → {} ready → {} after requests (+{}), peak {}\n\
         intelephense {intelephense_version}: index {:.1} s; working set {} after indexing, peak {}\n",
        indexed.as_secs_f64(),
        mb(before.current),
        mb(ready.current),
        mb(working.current),
        mb(working.current.saturating_sub(before.current)),
        peak(working),
        intelephense_indexed.as_secs_f64(),
        mb(intelephense.current),
        peak(intelephense),
    );
    println!("{report}");
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target");
    std::fs::write(target.join("php-memory.txt"), &report).unwrap();
    let unix_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let seconds = |d: Duration| (d.as_secs_f64() * 10.0).round() / 10.0;
    let json = json!({
        "unix_time": unix_time,
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "threads": std::thread::available_parallelism().map_or(0, |n| n.get()),
        "release": !cfg!(debug_assertions),
        "corpus": "laravel-framework",
        "files": files,
        "engine": {
            "index_s": seconds(indexed),
            "before_mb": megabytes(before.current),
            "ready_mb": megabytes(ready.current),
            "after_requests_mb": megabytes(working.current),
            "peak_mb": working.peak.map(megabytes),
        },
        "intelephense": {
            "version": intelephense_version,
            "index_s": seconds(intelephense_indexed),
            "after_indexing_mb": megabytes(intelephense.current),
            "peak_mb": intelephense.peak.map(megabytes),
        },
    });
    std::fs::write(target.join("php-memory.json"), serde_json::to_string_pretty(&json).unwrap() + "\n").unwrap();
}
