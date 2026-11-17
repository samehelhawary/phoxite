//! The command line: the language server (the default), or `check`, which
//! inspects a project and prints its problems.

use std::path::PathBuf;

pub const USAGE: &str = "\
A language server for PHP and Laravel. It talks LSP over stdin/stdout.

Usage:
  phoxite-lsp [--stdio]        run the language server (the default)
  phoxite-lsp check [PATH...]  inspect a project and print its problems
  phoxite-lsp --version

Environment:
  PHOXITE_LOG   log filter for stderr, e.g. `debug` (default `info`; `warn` for check)

`phoxite-lsp check --help` says more about check.";

pub const CHECK_USAGE: &str = "\
Inspects a project's own PHP files and Blade templates (not vendor/) and prints
their problems, as the language server reports them in an editor.

Usage: phoxite-lsp check [OPTIONS] [PATH...]

The project is the nearest folder with a phoxite.json or a composer.json, from
the first PATH up (from the current folder without a PATH). Only the files at
or below a PATH are reported; all of the project's, without one.

Options:
  --format <text|json|sarif|github>   how the problems are printed (default text);
                                      github: workflow commands, which GitHub
                                      Actions shows as annotations
  --fail-on <error|warning|never>     the least severe problem that makes the exit
                                      code 1 (default warning)
  --config <FILE>                     settings from FILE instead of the project's
                                      phoxite.json
  --no-config                         the default settings, whatever the project has
  --php-version <X.Y>                 check against this PHP version
  --cache-dir <DIR>                   where the indexes are kept (default: the
                                      language server's, so an index it built is used)
  --no-cache                          index from scratch and keep nothing

Exit codes: 0 no problem at the --fail-on level, 1 problems, 2 the command line
or the settings are wrong, 3 the project couldn't be indexed.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Sarif,
    Github,
}

/// The least severe problem that fails the check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailOn {
    Error,
    Warning,
    Never,
}

/// Where `check` takes its settings from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Config {
    /// The project's `phoxite.json`, when it has one.
    Project,
    File(PathBuf),
    /// The defaults.
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckOptions {
    pub paths: Vec<PathBuf>,
    pub format: Format,
    pub fail_on: FailOn,
    pub config: Config,
    pub php_version: Option<String>,
    pub cache_dir: Option<PathBuf>,
    pub no_cache: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Serve,
    Version,
    Help,
    CheckHelp,
    Check(CheckOptions),
}

/// What the arguments (without the program's name) ask for; an error is a
/// sentence for the user.
///
/// The server takes any argument: clients pass `--stdio` and some pass more,
/// and a release keeps starting for them (`docs/openidea.md`). `check` is
/// typed by a person or written in a workflow, so there a slip is an error.
pub fn parse(args: &[String]) -> Result<Command, String> {
    if args.first().map(String::as_str) != Some("check") {
        return Ok(if args.iter().any(|a| a == "--version" || a == "-V") {
            Command::Version
        } else if args.iter().any(|a| a == "--help" || a == "-h") {
            Command::Help
        } else {
            Command::Serve
        });
    }
    let mut options = CheckOptions {
        paths: Vec::new(),
        format: Format::Text,
        fail_on: FailOn::Warning,
        config: Config::Project,
        php_version: None,
        cache_dir: None,
        no_cache: false,
    };
    let mut args = args[1..].iter();
    let mut only_paths = false;
    while let Some(arg) = args.next() {
        if only_paths || !arg.starts_with('-') {
            options.paths.push(PathBuf::from(arg));
            continue;
        }
        // `--name value` and `--name=value`.
        let (name, inline) = match arg.split_once('=') {
            Some((name, value)) => (name, Some(value.to_owned())),
            None => (arg.as_str(), None),
        };
        let mut value = || inline.clone().or_else(|| args.next().cloned()).ok_or(format!("{name} needs a value"));
        match name {
            "--" => only_paths = true,
            "--help" | "-h" => return Ok(Command::CheckHelp),
            "--version" | "-V" => return Ok(Command::Version),
            "--format" => {
                options.format = match value()?.as_str() {
                    "text" => Format::Text,
                    "json" => Format::Json,
                    "sarif" => Format::Sarif,
                    "github" => Format::Github,
                    other => return Err(format!("--format {other}: text, json, sarif or github")),
                }
            }
            "--fail-on" => {
                options.fail_on = match value()?.as_str() {
                    "error" => FailOn::Error,
                    "warning" => FailOn::Warning,
                    "never" => FailOn::Never,
                    other => return Err(format!("--fail-on {other}: error, warning or never")),
                }
            }
            "--config" => options.config = Config::File(PathBuf::from(value()?)),
            "--no-config" => options.config = Config::None,
            "--php-version" => options.php_version = Some(value()?),
            "--cache-dir" => options.cache_dir = Some(PathBuf::from(value()?)),
            "--no-cache" => options.no_cache = true,
            other => return Err(format!("unknown option {other}")),
        }
    }
    if options.no_cache && options.cache_dir.is_some() {
        return Err("--no-cache and --cache-dir don't go together".to_owned());
    }
    Ok(Command::Check(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(args: &[&str]) -> Result<Command, String> {
        parse(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    fn check(args: &[&str]) -> CheckOptions {
        match parsed(args) {
            Ok(Command::Check(options)) => options,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_server_starts_whatever_a_client_passes() {
        assert_eq!(parsed(&[]), Ok(Command::Serve));
        assert_eq!(parsed(&["--stdio"]), Ok(Command::Serve));
        assert_eq!(parsed(&["--stdio", "--clientProcessId=41", "something"]), Ok(Command::Serve));
        assert_eq!(parsed(&["--stdio", "--version"]), Ok(Command::Version));
        assert_eq!(parsed(&["-V"]), Ok(Command::Version));
        assert_eq!(parsed(&["--stdio", "-h"]), Ok(Command::Help));
        // `check` is the command only as the first argument.
        assert_eq!(parsed(&["--stdio", "check"]), Ok(Command::Serve));
    }

    #[test]
    fn check_takes_paths_and_options() {
        let defaults = check(&["check"]);
        assert_eq!(
            (defaults.format, defaults.fail_on, &defaults.config),
            (Format::Text, FailOn::Warning, &Config::Project)
        );
        assert!(defaults.paths.is_empty() && !defaults.no_cache && defaults.cache_dir.is_none());

        let options = check(&[
            "check",
            "app",
            "--format",
            "sarif",
            "--fail-on=error",
            "routes/web.php",
            "--php-version",
            "8.2",
            "--config=ci/phoxite.json",
            "--cache-dir",
            ".cache",
        ]);
        assert_eq!(options.paths, [PathBuf::from("app"), PathBuf::from("routes/web.php")]);
        assert_eq!((options.format, options.fail_on), (Format::Sarif, FailOn::Error));
        assert_eq!(options.config, Config::File(PathBuf::from("ci/phoxite.json")));
        assert_eq!(options.php_version.as_deref(), Some("8.2"));
        assert_eq!(options.cache_dir, Some(PathBuf::from(".cache")));

        assert_eq!(check(&["check", "--no-config", "--no-cache"]).config, Config::None);
        // After `--` everything is a path.
        assert_eq!(check(&["check", "--", "--odd-name"]).paths, [PathBuf::from("--odd-name")]);
        assert_eq!(parsed(&["check", "app", "--help"]), Ok(Command::CheckHelp));
    }

    #[test]
    fn a_slip_in_check_is_an_error() {
        assert_eq!(parsed(&["check", "--fromat", "json"]), Err("unknown option --fromat".into()));
        assert_eq!(parsed(&["check", "--format"]), Err("--format needs a value".into()));
        assert_eq!(parsed(&["check", "--format", "xml"]), Err("--format xml: text, json, sarif or github".into()));
        assert_eq!(parsed(&["check", "--fail-on", "hint"]), Err("--fail-on hint: error, warning or never".into()));
        assert!(parsed(&["check", "--no-cache", "--cache-dir", "x"]).is_err());
    }
}
