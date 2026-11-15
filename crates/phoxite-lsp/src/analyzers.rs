//! PHPStan and Psalm on save.
//!
//! With `analyzers.onSave` on, each saved PHP file of a project (not a Blade
//! view, not a dependency) is analysed by the tools the project installs in
//! `vendor/bin` (`phoxite::analyzers::detect`), run with the interpreter of
//! `analyzers.phpPath` on a thread of their own. Their problems are published
//! under the tool's own source (`phpstan`, `psalm`), minus those the engine
//! already reports for the file. A newer save of the same file cancels a run in
//! progress; closing the file and turning the setting off clear what was
//! published.
//!
//! The tools run code of the project (their configuration, their extensions),
//! so nothing runs in a workspace the client says it doesn't trust
//! (`initializationOptions.trusted`, then `phoxite/workspaceTrust`).

use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lsp_types as lsp;
use phoxite::analyzers::{self, Analyzer};
use phoxite::base::CancellationToken;
use phoxite::engine::PhpEngine;
use serde::Deserialize;

use crate::state::{Kind, State};

/// Client notification: whether the workspace is trusted, when that changes.
pub const WORKSPACE_TRUST: &str = "phoxite/workspaceTrust";

/// The longest one analyser may run on a file.
const TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Deserialize)]
pub struct TrustParams {
    pub trusted: bool,
}

#[derive(Default)]
pub struct Analyzers {
    /// The run in progress for each file, with its id.
    running: HashMap<PathBuf, (u64, CancellationToken)>,
    next_run: u64,
    /// Files with problems published, by source.
    published: HashSet<(PathBuf, &'static str)>,
    /// Analysers whose failure was already shown.
    failed: HashSet<&'static str>,
}

impl Analyzers {
    /// Stops the run on `path`. The caller drops the file's problems.
    pub fn closed(&mut self, path: &Path) {
        if let Some((_, token)) = self.running.remove(path) {
            token.cancel();
        }
        self.published.retain(|(published, _)| published != path);
    }
}

/// Whether a saved file is one the analysers look at.
fn analysed(path: &Path, root: &Path) -> bool {
    let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    name.ends_with(".php")
        && !name.ends_with(".blade.php")
        && path.strip_prefix(root).is_ok_and(|rel| !rel.starts_with("vendor"))
}

/// Stops every run and clears what the analysers published (the setting was
/// turned off, or the workspace is no longer trusted).
pub fn clear_all(state: &State) {
    let mut runs = state.analyzers.lock();
    for (_, (_, token)) in runs.running.drain() {
        token.cancel();
    }
    let published: Vec<_> = runs.published.drain().collect();
    // Published under the lock, so a run's result can't overtake the clear.
    for (path, source) in published {
        state.problems_changed(&path, source.to_owned(), Vec::new());
    }
}

/// The open document at `uri` was saved: analyses it in the background.
pub fn saved(state: &Arc<State>, uri: &lsp::Url) {
    let Some(doc) = state.snapshot(uri) else { return };
    let Some(engine) = doc.engine.clone() else { return };
    if doc.kind != Kind::Php || !analysed(&doc.path, &doc.root) {
        return;
    }
    let (on_save, interpreter) = {
        let settings = state.settings.read();
        let configured = settings.analyzers.php_path.trim();
        (settings.analyzers.on_save, PathBuf::from(if configured.is_empty() { "php" } else { configured }))
    };
    if !on_save || !state.is_trusted() {
        return;
    }
    let tools = analyzers::detect(&doc.root);
    if tools.is_empty() {
        return;
    }
    let token = CancellationToken::new();
    let id = {
        let mut runs = state.analyzers.lock();
        runs.next_run += 1;
        let id = runs.next_run;
        if let Some((_, previous)) = runs.running.insert(doc.path.clone(), (id, token.clone())) {
            previous.cancel();
        }
        id
    };
    let job = Job {
        state: state.clone(),
        engine,
        root: doc.root.clone(),
        path: doc.path.clone(),
        text: doc.text.to_string(),
        interpreter,
        tools,
        id,
        token,
    };
    if let Err(e) = std::thread::Builder::new().name("phoxite-analyzers".into()).spawn(move || job.run()) {
        tracing::warn!("analysers: {e}");
    }
}

/// One save's run of every installed analyser.
struct Job {
    state: Arc<State>,
    engine: Arc<PhpEngine>,
    root: PathBuf,
    path: PathBuf,
    text: String,
    interpreter: PathBuf,
    tools: Vec<Analyzer>,
    id: u64,
    token: CancellationToken,
}

impl Job {
    fn run(self) {
        for tool in &self.tools {
            if self.token.is_cancelled() {
                break;
            }
            let found = execute(&self.interpreter, *tool, &self.root, &self.path, &self.token, TIMEOUT)
                .and_then(|out| tool.parse(&out, &self.path, &self.root, &self.text));
            if self.token.is_cancelled() {
                break;
            }
            match found {
                Ok(mut items) => {
                    // Drops what the engine's latest inspection reports too.
                    if let Some(document) = self.engine.open_document(&self.path)
                        && let Some(inspected) = self.engine.inspected(document.doc)
                    {
                        analyzers::remove_duplicates(&mut items, &inspected.diagnostics);
                    }
                    let mut runs = self.state.analyzers.lock();
                    if !self.current(&runs) {
                        break;
                    }
                    runs.failed.remove(tool.source());
                    runs.published.insert((self.path.clone(), tool.source()));
                    // Published under the lock, so a clear can't be overtaken.
                    self.state.problems_changed(&self.path, tool.source().to_owned(), items);
                }
                Err(error) => {
                    tracing::warn!("{} on {}: {error}", tool.source(), self.path.display());
                    if self.state.analyzers.lock().failed.insert(tool.source()) {
                        self.state.notify::<lsp::notification::ShowMessage>(lsp::ShowMessageParams {
                            typ: lsp::MessageType::WARNING,
                            message: format!("{} could not analyse the file: {error}", tool.source()),
                        });
                    }
                }
            }
        }
        let mut runs = self.state.analyzers.lock();
        if self.current(&runs) {
            runs.running.remove(&self.path);
        }
    }

    fn current(&self, runs: &Analyzers) -> bool {
        !self.token.is_cancelled() && runs.running.get(&self.path).is_some_and(|(id, _)| *id == self.id)
    }
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs `php vendor/bin/<tool> <args> <file>` in `root` and returns its
/// stdout. Killed when `cancel` fires or after `timeout`.
fn execute(
    interpreter: &Path,
    tool: Analyzer,
    root: &Path,
    file: &Path,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Result<String, String> {
    let mut cmd = Command::new(interpreter);
    #[cfg(windows)]
    {
        // No console window flashing up for each run.
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.arg(tool.script(root))
        .args(tool.args(file))
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("cannot start {}: {e}", interpreter.display()))?;
    let reader = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    };
    let stdout = reader(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let stderr = reader(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    let started = Instant::now();
    loop {
        if cancel.is_cancelled() {
            kill(&mut child);
            return Err("cancelled".into());
        }
        if started.elapsed() > timeout {
            kill(&mut child);
            return Err(format!("{} timed out after {} s", tool.source(), timeout.as_secs()));
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                kill(&mut child);
                return Err(e.to_string());
            }
        }
    }
    let out = stdout.join().unwrap_or_default();
    let err = stderr.join().unwrap_or_default();
    if out.trim().is_empty() && !err.trim().is_empty() {
        // Nothing to parse: the reason is on stderr (no PHP, a broken config).
        return Err(err.trim().chars().take(2000).collect());
    }
    Ok(out)
}
