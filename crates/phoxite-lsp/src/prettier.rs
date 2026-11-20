//! Format Document for Twig templates with the project's Prettier.
//!
//! With `format.twig` set to `prettier`, a whole Twig template of a project that
//! installs Prettier (`node_modules/prettier`) is formatted by it: the document's text
//! goes to Prettier's standard input with the template's path as `--stdin-filepath`, so
//! that the project's Prettier configuration and its Twig plugin apply, and what it
//! prints comes back as the edit. The document may have unsaved changes, and the
//! project's own file is never touched.
//!
//! Prettier reformats what the built-in formatter leaves alone: it rewraps long lines
//! and formats the JavaScript and CSS inside the template. It needs a plugin that reads
//! Twig (`@zackad/prettier-plugin-twig`, say) in the project's Prettier configuration;
//! without one Prettier says it can't infer a parser, which is told like any failure.
//!
//! Prettier runs code of the project (its configuration and plugins), so like Pint and
//! the analysers it runs only in a workspace the client says it trusts. Without trust,
//! without Prettier, for a range, and when Prettier fails, the built-in formatter
//! answers.

use std::path::{Path, PathBuf};
use std::time::Duration;

use phoxite::base::CancellationToken;

use crate::analyzers::run_program;
use crate::state::{DocSnapshot, State};

/// The longest Prettier may take on one file.
const TIMEOUT: Duration = Duration::from_secs(30);

/// What the failures are remembered under: each kind is told once.
const NOT_INSTALLED: &str = "prettier: not installed";
const FAILED: &str = "prettier: failed";

/// Prettier's command-line script, by where its releases put it (3, then 2).
const SCRIPTS: [&str; 2] = ["bin/prettier.cjs", "bin-prettier.js"];

/// Prettier's script for the file at `path` of the project at `root`: the one of the
/// nearest `node_modules/prettier` from the file's folder up to the project's.
pub fn script(root: &Path, path: &Path) -> Option<PathBuf> {
    let folders = path.ancestors().skip(1).take_while(|folder| folder.starts_with(root));
    folders.chain(std::iter::once(root)).find_map(|folder| {
        let package = folder.join("node_modules").join("prettier");
        SCRIPTS.iter().map(|script| package.join(script)).find(|script| script.is_file())
    })
}

/// The text of `doc` as Prettier formats it, when Prettier is the formatter of Twig
/// templates and can run; `None` leaves the document to the built-in formatter. What
/// stops Prettier is told to the user once: that the project doesn't install it, or why
/// it failed.
pub fn format(state: &State, doc: &DocSnapshot) -> Option<String> {
    let (wanted, node) = {
        // The program is the client's to name, the formatter the project's too.
        let settings = state.settings.read();
        let configured = settings.analyzers.node_path.trim();
        let wanted = state.settings_for(&doc.path).format.twig == "prettier";
        (wanted, PathBuf::from(if configured.is_empty() { "node" } else { configured }))
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
    let Some(script) = script(&doc.root, &doc.path) else {
        tell(
            NOT_INSTALLED,
            "`phoxite.format.twig` is `prettier`, but this project doesn't install Prettier (node_modules/prettier): the built-in formatter is used.".to_owned(),
        );
        return None;
    };
    let args = [script.into_os_string(), "--stdin-filepath".into(), doc.path.clone().into_os_string()];
    let ran = run_program(
        &node,
        &args,
        &doc.root,
        Some(doc.text.to_string()),
        &CancellationToken::new(),
        TIMEOUT,
        "Prettier",
    );
    let formatted = match ran {
        Ok((formatted, true)) => Ok(formatted),
        // What it printed before ending badly is no formatted template.
        Ok((said, false)) => Err(said.trim().chars().take(2000).collect::<String>()),
        Err(error) => Err(error),
    };
    match formatted {
        Ok(formatted) => {
            state.analyzers.lock().failed.remove(FAILED);
            Some(formatted)
        }
        Err(error) => {
            tracing::warn!("prettier on {}: {error}", doc.path.display());
            tell(FAILED, format!("Prettier could not format the template, so the built-in formatter is used: {error}"));
            None
        }
    }
}
