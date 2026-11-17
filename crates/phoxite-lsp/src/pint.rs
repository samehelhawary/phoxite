//! Format Document with Laravel Pint.
//!
//! With `format.tool` set to `pint`, a whole PHP file of a project that installs Pint
//! (`vendor/bin/pint`) is formatted by it: the document's text goes into a file of the
//! system's temporary folder, Pint (run with the interpreter of `analyzers.phpPath`, in
//! the project, so that its `pint.json` applies) rewrites that file, and what it wrote
//! comes back as the edit. The document may have unsaved changes, and the project's own
//! file is never touched.
//!
//! Pint runs code of the project (its configuration can name custom fixers), so like
//! the analysers it runs only in a workspace the client says it trusts. Without trust,
//! without Pint, for a range, and when Pint fails, the built-in formatter answers.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use phoxite::base::CancellationToken;

use crate::analyzers::run_php;
use crate::state::{DocSnapshot, State};

/// The longest Pint may take on one file.
const TIMEOUT: Duration = Duration::from_secs(30);

/// What the failures are remembered under: each kind is told once.
const NOT_INSTALLED: &str = "pint: not installed";
const FAILED: &str = "pint: failed";

/// Pint's script in the project at `root`, when the project installs it.
pub fn script(root: &Path) -> Option<PathBuf> {
    let script = root.join("vendor").join("bin").join("pint");
    script.is_file().then_some(script)
}

/// A folder of its own for one run, so that two runs never share a file.
fn scratch_dir() -> std::io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = format!("phoxite-pint-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `text` as Pint formats it in the project at `root`.
fn run(interpreter: &Path, script: &Path, root: &Path, file_name: &str, text: &str) -> Result<String, String> {
    let dir = scratch_dir().map_err(|e| format!("no temporary folder: {e}"))?;
    let file = dir.join(file_name);
    let formatted = std::fs::write(&file, text)
        .map_err(|e| e.to_string())
        .and_then(|()| {
            let args = [file.as_os_str().to_owned()];
            run_php(interpreter, script, &args, root, &CancellationToken::new(), TIMEOUT, "Pint")
        })
        .and_then(|(said, well)| {
            // What Pint prints is its report; when it ends badly, that is the reason.
            if well { Ok(()) } else { Err(said.trim().chars().take(2000).collect()) }
        })
        .and_then(|()| std::fs::read_to_string(&file).map_err(|e| e.to_string()));
    let _ = std::fs::remove_dir_all(&dir);
    formatted
}

/// The text of `doc` as Pint formats it, when Pint is the formatter and can run;
/// `None` leaves the document to the built-in formatter. What stops Pint is told to the
/// user once: that the project doesn't install it, or why it failed.
pub fn format(state: &State, doc: &DocSnapshot) -> Option<String> {
    let (wanted, interpreter) = {
        // The interpreter is the client's to name, the formatter the project's too.
        let settings = state.settings.read();
        let configured = settings.analyzers.php_path.trim();
        let wanted = state.settings_for(&doc.path).format.tool == "pint";
        (wanted, PathBuf::from(if configured.is_empty() { "php" } else { configured }))
    };
    if !wanted || !state.is_trusted() {
        return None;
    }
    let tell = |kind: &'static str, message: String| {
        if state.analyzers.lock().failed.insert(kind) {
            state.notify::<lsp_types::notification::ShowMessage>(lsp_types::ShowMessageParams {
                typ: lsp_types::MessageType::WARNING,
                message,
            });
        }
    };
    let Some(script) = script(&doc.root) else {
        tell(
            NOT_INSTALLED,
            "`phoxite.format.tool` is `pint`, but this project doesn't install Laravel Pint (vendor/bin/pint): the built-in formatter is used.".to_owned(),
        );
        return None;
    };
    let name = doc.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file.php".to_owned());
    match run(&interpreter, &script, &doc.root, &name, &doc.text) {
        Ok(formatted) => {
            state.analyzers.lock().failed.remove(FAILED);
            Some(formatted)
        }
        Err(error) => {
            tracing::warn!("pint on {}: {error}", doc.path.display());
            tell(FAILED, format!("Pint could not format the file, so the built-in formatter is used: {error}"));
            None
        }
    }
}
