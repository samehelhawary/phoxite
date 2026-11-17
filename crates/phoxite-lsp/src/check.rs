//! `phoxite-lsp check`: inspects a project's own files as the server does for
//! an editor, prints the problems ([`crate::report`]) and says by its exit
//! code whether there are any.
//!
//! The settings are the project's `phoxite.json` over the defaults: no
//! editor is there to send any. The live database of `laravel.database`
//! and the generator templates aren't read: neither changes a problem.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use parking_lot::Mutex;
use phoxite::base::{CancellationToken, Diagnostic, IdeEvent, Severity, SharedSink};
use phoxite::engine::{EngineState, PhpEngine};
use phoxite::inspect::InspectionConfig;
use phoxite::project::Origin;
use serde_json::Value;

use crate::cli::{CheckOptions, Config, FailOn};
use crate::config::{PROJECT_FILE, Settings, default_cache_dir, read_project_file, read_settings_file};
use crate::report::Report;
use crate::state::engine_config;

/// A project that takes longer to index than this is stuck.
const INDEX_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// The exit codes.
pub const CLEAN: u8 = 0;
pub const PROBLEMS: u8 = 1;
/// The command line or the settings are wrong.
pub const USAGE: u8 = 2;
/// The project couldn't be indexed or inspected.
pub const FAILED: u8 = 3;

/// Why a check didn't run to its end: the exit code and what to tell.
struct Stop(u8, String);

/// Runs the check, prints the report to stdout and what else there is to
/// say to stderr, and returns the exit code.
pub fn run(options: &CheckOptions) -> u8 {
    match check(options) {
        Ok(report) => {
            print!("{}", report.render(options.format));
            eprintln!("{}", report.summary());
            exit_code(&report, options.fail_on)
        }
        Err(Stop(code, message)) => {
            eprintln!("phoxite-lsp check: {message}");
            code
        }
    }
}

fn exit_code(report: &Report, fail_on: FailOn) -> u8 {
    let counts = report.counts();
    let failed = match fail_on {
        FailOn::Error => counts.error > 0,
        FailOn::Warning => counts.error + counts.warning > 0,
        FailOn::Never => false,
    };
    if failed { PROBLEMS } else { CLEAN }
}

/// The nearest folder with a settings file or a `composer.json`, from
/// `dir` up.
fn project_above(dir: &Path) -> Option<&Path> {
    dir.ancestors().find(|dir| dir.join(PROJECT_FILE).is_file() || dir.join("composer.json").is_file())
}

/// The folder and the files a check run in `cwd` is about, with the paths
/// as the index spells them (canonical, as the folder the engine is opened
/// on).
///
/// The project is the one the command is run in when the paths are in it:
/// in a repository of several packages, `check packages/a` at its root is
/// about the repository, as `check` there is. Run from elsewhere, it is the
/// project of the first path, and the other paths have to be in it. A
/// folder without a project is one itself.
fn project(cwd: &Path, paths: &[PathBuf]) -> Result<(PathBuf, Vec<PathBuf>), Stop> {
    let mut wanted = Vec::new();
    for path in paths {
        let found = dunce::canonicalize(cwd.join(path)).map_err(|e| Stop(USAGE, format!("{}: {e}", path.display())))?;
        wanted.push(found);
    }
    let here = project_above(cwd).unwrap_or(cwd);
    if wanted.iter().all(|path| path.starts_with(here)) {
        return Ok((here.to_path_buf(), wanted));
    }
    let first = &wanted[0];
    let start = if first.is_dir() { first.as_path() } else { first.parent().unwrap_or(first) };
    let root = project_above(start).unwrap_or(start);
    if let Some(outside) = wanted.iter().find(|path| !path.starts_with(root)) {
        let message = format!("{} is outside the project at {}", outside.display(), root.display());
        return Err(Stop(USAGE, message));
    }
    Ok((root.to_path_buf(), wanted))
}

fn settings(options: &CheckOptions, root: &Path) -> Result<Settings, Stop> {
    let (file, name) = match &options.config {
        Config::Project => (read_project_file(root), root.join(PROJECT_FILE)),
        Config::File(path) => (read_settings_file(path).map(Some), path.clone()),
        Config::None => (Ok(None), PathBuf::new()),
    };
    let file = file.map_err(|e| Stop(USAGE, format!("{}: {e}", name.display())))?;
    // Whoever runs the check on a project chose to: its include paths are read.
    let (mut settings, rejected) = Settings::layered(&Value::Null, file.as_ref(), true);
    if !rejected.is_empty() {
        let names: Vec<String> = rejected.iter().map(|path| format!("`{path}`")).collect();
        return Err(Stop(
            USAGE,
            format!("{}: the value of {} isn't one Phoxite understands", name.display(), names.join(", ")),
        ));
    }
    if let Some(version) = &options.php_version {
        settings.php_version = version.clone();
    }
    Ok(settings)
}

fn check(options: &CheckOptions) -> Result<Report, Stop> {
    let cwd = std::env::current_dir()
        .and_then(dunce::canonicalize)
        .map_err(|e| Stop(USAGE, format!("the current folder: {e}")))?;
    let (root, wanted) = project(&cwd, &options.paths)?;
    let settings = settings(options, &root)?;
    let php = settings.engine_config();

    // Without a cache the index is built in a folder of its own, removed at the end.
    static RUNS: AtomicU32 = AtomicU32::new(0);
    let scratch = options.no_cache.then(|| {
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("phoxite-check-{}-{run}", std::process::id()))
    });
    let cache = scratch.clone().or_else(|| options.cache_dir.clone()).unwrap_or_else(default_cache_dir);
    let sink: SharedSink = Arc::new(|_: IdeEvent| {});
    let engine = PhpEngine::open(engine_config(&cache, &root, &php), sink)
        .map_err(|e| Stop(FAILED, format!("can't start on {}: {e}", root.display())))?;
    let found = inspect(&engine, &settings, &root, &wanted);
    engine.shutdown();
    drop(engine);
    if let Some(scratch) = scratch {
        let _ = std::fs::remove_dir_all(scratch);
    }
    found
}

fn inspect(engine: &PhpEngine, settings: &Settings, root: &Path, wanted: &[PathBuf]) -> Result<Report, Stop> {
    let php = settings.engine_config();
    engine.configure_inspections(InspectionConfig::from_profile(&php.inspections));
    engine.set_laravel_settings(php.laravel.clone());
    if !engine.wait_idle(INDEX_TIMEOUT) {
        return Err(Stop(FAILED, format!("indexing {} took too long", root.display())));
    }
    let mut rules: Vec<(String, Severity)> = engine
        .inspections()
        .iter()
        .map(|inspection| (inspection.id().to_owned(), inspection.default_severity()))
        .collect();
    let report = |inspected, files| Report { root: root.to_path_buf(), inspected, files, rules: Vec::new() };
    match engine.status().state {
        EngineState::Ready => {}
        // Nothing to index: nothing to report either.
        EngineState::Inactive => return Ok(Report { rules, ..report(0, Vec::new()) }),
        EngineState::Failed(message) => return Err(Stop(FAILED, format!("indexing failed: {message}"))),
        other => return Err(Stop(FAILED, format!("the engine is {other:?}"))),
    }
    if !engine.status().persistent {
        tracing::warn!("the index is in use (an editor has the project open?): indexed in memory");
    }

    let cancel = CancellationToken::new();
    let layout = engine.layout().ok_or_else(|| Stop(FAILED, "the project has no layout".to_owned()))?;
    let files: Vec<PathBuf> = layout
        .files(&cancel)
        .map_err(|e| Stop(FAILED, e.to_string()))?
        .into_iter()
        .filter(|(_, origin)| *origin == Origin::Project)
        .map(|(path, _)| path)
        .filter(|path| wanted.is_empty() || wanted.iter().any(|folder| path.starts_with(folder)))
        .collect();

    let found: Mutex<Vec<(PathBuf, Vec<Diagnostic>)>> = Mutex::default();
    let done = engine.inspect_files(
        &files,
        |path| path.as_path(),
        |path| std::fs::read(path).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
        &cancel,
        |path, problems| {
            let mut problems = problems.unwrap_or_default();
            if problems.is_empty() {
                return;
            }
            for problem in &mut problems {
                settings.apply_severity(problem);
            }
            problems.sort_by_key(|problem| problem.range.start);
            found.lock().push((path.clone(), problems));
        },
    );
    if !done {
        return Err(Stop(FAILED, "the inspection was cancelled".to_owned()));
    }
    let mut found = found.into_inner();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    // A Blade template's problems have rules no inspection declares.
    for problem in found.iter().flat_map(|(_, problems)| problems) {
        if let Some(code) = &problem.code
            && !rules.iter().any(|(id, _)| id == code)
        {
            rules.push((code.clone(), problem.severity));
        }
    }
    Ok(Report { rules, ..report(files.len(), found) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Format;

    /// A project with an undefined class, an unused import and a file in
    /// `vendor/` that would be reported if it were the project's.
    fn project_with_problems() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        let write = |rel: &str, text: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        write("composer.json", r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#);
        write("src/Broken.php", "<?php\nnamespace App;\n\nfunction f(): void {\n    new UnknownThing();\n}\n");
        write("src/Deep/Unused.php", "<?php\nnamespace App\\Deep;\n\nuse App\\Fine;\n\nclass Unused {}\n");
        write("src/Fine.php", "<?php\nnamespace App;\n\nclass Fine {}\n");
        write(
            "src/Quiet.php",
            "<?php\nnamespace App;\n\nfunction g(): void {\n    /** @noinspection PhpUndefinedClassInspection */\n    new AnotherUnknown();\n}\n",
        );
        write("vendor/acme/lib/src/Bad.php", "<?php\nnamespace Acme;\n\nfunction h(): void {\n    new Missing();\n}\n");
        (dir, root)
    }

    fn options(paths: &[&Path]) -> CheckOptions {
        CheckOptions {
            paths: paths.iter().map(|path| path.to_path_buf()).collect(),
            format: Format::Text,
            fail_on: FailOn::Warning,
            config: Config::Project,
            php_version: None,
            cache_dir: None,
            no_cache: true,
        }
    }

    fn found(report: &Report) -> Vec<String> {
        report.render(Format::Text).lines().map(str::to_owned).collect()
    }

    #[test]
    fn the_projects_own_problems_are_reported() {
        let (_dir, root) = project_with_problems();
        let report = check(&options(&[&root])).unwrap_or_else(|Stop(_, message)| panic!("{message}"));
        assert_eq!(report.inspected, 4, "not the file in vendor/");
        let found = found(&report);
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found[0].starts_with("src/Broken.php:5:9: ") && found[0].contains("[PhpUndefinedClass]"), "{found:?}");
        assert!(found[1].starts_with("src/Deep/Unused.php:4:") && found[1].contains("[PhpUnusedAlias]"), "{found:?}");
        assert!(report.rules.iter().any(|(id, _)| id == "PhpUndefinedClass"));

        // Below a path: only its files, in the same project.
        let deep = check(&options(&[&root.join("src/Deep")])).unwrap_or_else(|Stop(_, message)| panic!("{message}"));
        assert_eq!(deep.root, root);
        assert_eq!(deep.inspected, 1);
        assert_eq!(self::found(&deep).len(), 1);
        let one = check(&options(&[&root.join("src/Fine.php")])).unwrap_or_else(|Stop(_, message)| panic!("{message}"));
        assert_eq!((one.inspected, one.files.len()), (1, 0));
    }

    #[test]
    fn the_settings_file_and_the_options_decide_what_fails() {
        let (_dir, root) = project_with_problems();
        let run = |options: &CheckOptions| check(options).unwrap_or_else(|Stop(_, message)| panic!("{message}"));
        let all = options(&[&root]);
        let by_default = run(&all);
        let severities: Vec<Severity> = by_default.files.iter().map(|(_, problems)| problems[0].severity).collect();
        assert_eq!(exit_code(&by_default, FailOn::Never), CLEAN);
        assert_eq!(exit_code(&by_default, FailOn::Warning), PROBLEMS);

        // An inspection turned off is gone; a severity the file sets is the one reported.
        let file = r#"{ "inspections": { "PhpUnusedAlias": { "enabled": false }, "PhpUndefinedClass": { "severity": "hint" } } }"#;
        std::fs::write(root.join(PROJECT_FILE), file).unwrap();
        let configured = run(&all);
        assert_eq!(configured.files.len(), 1, "{:?}", found(&configured));
        assert_eq!(configured.files[0].1[0].severity, Severity::Hint);
        assert_eq!(exit_code(&configured, FailOn::Warning), CLEAN);

        // Without the file's settings, or with another file's.
        let ignored = run(&CheckOptions { config: Config::None, ..all.clone() });
        assert_eq!(ignored.files.iter().map(|(_, problems)| problems[0].severity).collect::<Vec<_>>(), severities);
        let other = root.join("ci.json");
        std::fs::write(&other, r#"{ "inspections": { "*": { "enabled": false } } }"#).unwrap();
        assert!(run(&CheckOptions { config: Config::File(other), ..all.clone() }).files.is_empty());

        // What is wrong with the settings or the paths stops the check.
        std::fs::write(root.join(PROJECT_FILE), r#"{ "phpVersion": 8 }"#).unwrap();
        let Err(Stop(code, message)) = check(&all) else { panic!("a broken settings file passed") };
        assert_eq!(code, USAGE);
        assert!(message.contains("phoxite.json") && message.contains("`phpVersion`"), "{message}");
        let Err(Stop(code, _)) = check(&options(&[&root.join("nowhere")])) else { panic!("a missing path passed") };
        assert_eq!(code, USAGE);
    }

    #[test]
    fn the_php_version_of_the_command_line_wins() {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join("composer.json"), r#"{ "require": { "php": "^8.4" } }"#).unwrap();
        std::fs::write(root.join("index.php"), "<?php\nclass C {\n    public string $p { get => 'x'; }\n}\n").unwrap();
        let all = options(&[&root]);
        assert!(check(&all).unwrap_or_else(|Stop(_, m)| panic!("{m}")).files.is_empty());
        let older =
            check(&CheckOptions { php_version: Some("8.1".into()), ..all }).unwrap_or_else(|Stop(_, m)| panic!("{m}"));
        assert_eq!(found(&older).len(), 1, "{:?}", found(&older));
        assert!(found(&older)[0].contains("[PhpLanguageLevel]"));
    }

    #[test]
    fn the_project_is_the_one_the_command_is_run_in() {
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        for folder in ["src", "packages/a/src", "packages/b", "elsewhere/loose"] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
        }
        for manifest in ["composer.json", "packages/a/composer.json", "packages/b/phoxite.json"] {
            std::fs::write(root.join(manifest), "{}").unwrap();
        }
        std::fs::write(root.join("packages/a/src/A.php"), "<?php\n").unwrap();
        let paths = |paths: &[&str]| paths.iter().map(PathBuf::from).collect::<Vec<_>>();
        let found =
            |cwd: &Path, given: &[&str]| project(cwd, &paths(given)).map(|(root, _)| root).map_err(|stop| stop.0);

        // At the root of a repository of packages: the repository, whatever is named.
        assert_eq!(found(&root, &[]), Ok(root.clone()));
        assert_eq!(found(&root, &["packages/a/src", "src"]), Ok(root.clone()));
        assert_eq!(found(&root, &["packages/a/src/A.php"]), Ok(root.clone()));
        // In a package: the package.
        assert_eq!(found(&root.join("packages/a/src"), &[]), Ok(root.join("packages/a")));
        assert_eq!(found(&root.join("packages/b"), &[]), Ok(root.join("packages/b")));
        // Paths outside where the command is run: the project of the first.
        let other = tempfile::tempdir().unwrap();
        let other = dunce::canonicalize(other.path()).unwrap();
        let a = root.join("packages/a/src/A.php");
        assert_eq!(found(&other, &[a.to_str().unwrap()]), Ok(root.join("packages/a")));
        assert_eq!(found(&other, &[a.to_str().unwrap(), root.join("src").to_str().unwrap()]), Err(USAGE));
        assert_eq!(found(&root.join("packages/a"), &["../b"]), Ok(root.join("packages/b")));
        // No project above: the folder itself.
        assert_eq!(found(&other, &[]), Ok(other.clone()));
        assert_eq!(found(&root, &["nowhere"]), Err(USAGE));
    }

    #[test]
    fn a_folder_without_php_has_nothing_to_report() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), "# nothing\n").unwrap();
        let report = check(&options(&[dir.path()])).unwrap_or_else(|Stop(_, message)| panic!("{message}"));
        assert_eq!((report.inspected, report.files.len()), (0, 0));
        assert_eq!(exit_code(&report, FailOn::Warning), CLEAN);
    }
}
