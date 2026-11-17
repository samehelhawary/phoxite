//! What `check` prints: the problems of a project as text, JSON, SARIF
//! 2.1.0 or GitHub workflow commands.
//!
//! Lines and columns are 1-based in every format. A column counts UTF-16
//! code units, as the engine and the editors do.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use phoxite::base::{Diagnostic, Severity};
use serde_json::{Value, json};

use crate::cli::Format;

/// The rule of a problem no inspection reported (a syntax error).
const SYNTAX_RULE: &str = "syntax";

/// What a check found.
pub struct Report {
    pub root: PathBuf,
    /// How many files were inspected.
    pub inspected: usize,
    /// The files with problems, sorted, and their problems in source order.
    pub files: Vec<(PathBuf, Vec<Diagnostic>)>,
    /// Every inspection that ran, with its default severity.
    pub rules: Vec<(String, Severity)>,
}

/// How many problems there are of each severity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub error: usize,
    pub warning: usize,
    pub info: usize,
    pub hint: usize,
}

impl Report {
    pub fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        for problem in self.files.iter().flat_map(|(_, problems)| problems) {
            match problem.severity {
                Severity::Error => counts.error += 1,
                Severity::Warning => counts.warning += 1,
                Severity::Info => counts.info += 1,
                Severity::Hint => counts.hint += 1,
            }
        }
        counts
    }

    /// `3 errors, 1 warning in 2 of 40 files`.
    pub fn summary(&self) -> String {
        let counts = self.counts();
        let total = counts.error + counts.warning + counts.info + counts.hint;
        if total == 0 {
            return format!("no problems in {}", files(self.inspected));
        }
        let parts: Vec<String> =
            [(counts.error, "error"), (counts.warning, "warning"), (counts.info, "note"), (counts.hint, "hint")]
                .into_iter()
                .filter(|(count, _)| *count > 0)
                .map(|(count, name)| format!("{count} {name}{}", if count == 1 { "" } else { "s" }))
                .collect();
        format!("{} in {} of {}", parts.join(", "), self.files.len(), files(self.inspected))
    }

    pub fn render(&self, format: Format) -> String {
        match format {
            Format::Text => self.text(),
            Format::Json => pretty(&self.json()),
            Format::Sarif => pretty(&self.sarif()),
            Format::Github => self.github(),
        }
    }

    /// The path as it is printed: relative to the project, with `/`.
    fn shown(&self, path: &Path) -> String {
        path.strip_prefix(&self.root).unwrap_or(path).to_string_lossy().replace('\\', "/")
    }

    fn problems(&self) -> impl Iterator<Item = (String, &Diagnostic)> {
        self.files.iter().flat_map(|(path, problems)| problems.iter().map(|problem| (self.shown(path), problem)))
    }

    fn text(&self) -> String {
        let mut out = String::new();
        for (file, p) in self.problems() {
            let (line, column) = (p.range.start.line + 1, p.range.start.col + 1);
            let _ = writeln!(out, "{file}:{line}:{column}: {} [{}] {}", name(p.severity), rule(p), p.message);
        }
        out
    }

    fn json(&self) -> Value {
        let counts = self.counts();
        let problems: Vec<Value> = self
            .problems()
            .map(|(file, p)| {
                json!({
                    "file": file,
                    "line": p.range.start.line + 1,
                    "column": p.range.start.col + 1,
                    "endLine": p.range.end.line + 1,
                    "endColumn": p.range.end.col + 1,
                    "severity": name(p.severity),
                    "code": rule(p),
                    "source": p.source.as_deref().unwrap_or("php"),
                    "message": p.message,
                })
            })
            .collect();
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "root": self.root.to_string_lossy().replace('\\', "/"),
            "files": self.inspected,
            "problems": problems,
            "summary": { "error": counts.error, "warning": counts.warning, "note": counts.info, "hint": counts.hint },
        })
    }

    /// One workflow command per problem. GitHub has three levels: notes and
    /// hints are both a notice.
    fn github(&self) -> String {
        let mut out = String::new();
        for (file, p) in self.problems() {
            let level = match p.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info | Severity::Hint => "notice",
            };
            let _ = writeln!(
                out,
                "::{level} file={},line={},col={},endLine={},endColumn={},title={}::{}",
                github_property(&file),
                p.range.start.line + 1,
                p.range.start.col + 1,
                p.range.end.line + 1,
                p.range.end.col + 1,
                github_property(rule(p)),
                github_data(&p.message),
            );
        }
        out
    }

    fn sarif(&self) -> Value {
        let mut rules: Vec<Value> = self
            .rules
            .iter()
            .map(|(id, severity)| json!({ "id": id, "defaultConfiguration": { "level": sarif_level(*severity) } }))
            .collect();
        rules.push(json!({ "id": SYNTAX_RULE, "defaultConfiguration": { "level": "error" } }));
        let results: Vec<Value> = self
            .problems()
            .map(|(file, p)| {
                json!({
                    "ruleId": rule(p),
                    "level": sarif_level(p.severity),
                    "message": { "text": p.message },
                    "locations": [{ "physicalLocation": {
                        "artifactLocation": { "uri": uri_path(&file), "uriBaseId": "SRCROOT" },
                        "region": {
                            "startLine": p.range.start.line + 1,
                            "startColumn": p.range.start.col + 1,
                            "endLine": p.range.end.line + 1,
                            "endColumn": p.range.end.col + 1,
                        },
                    } }],
                })
            })
            .collect();
        json!({
            "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
            "version": "2.1.0",
            "runs": [{
                "tool": { "driver": {
                    "name": "Phoxite",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/samehelhawary/phoxite",
                    "rules": rules,
                } },
                "columnKind": "utf16CodeUnits",
                "originalUriBaseIds": { "SRCROOT": { "uri": root_uri(&self.root) } },
                "results": results,
            }],
        })
    }
}

fn files(count: usize) -> String {
    format!("{count} file{}", if count == 1 { "" } else { "s" })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("JSON values print") + "\n"
}

fn rule(problem: &Diagnostic) -> &str {
    problem.code.as_deref().unwrap_or(SYNTAX_RULE)
}

fn name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
        Severity::Hint => "hint",
    }
}

/// SARIF has no level below a note.
fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info | Severity::Hint => "note",
    }
}

/// The message of a workflow command: `%` and line ends are escaped.
fn github_data(text: &str) -> String {
    text.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A")
}

/// A property of a workflow command: `:` and `,` end one, so they are
/// escaped too.
fn github_property(text: &str) -> String {
    github_data(text).replace(':', "%3A").replace(',', "%2C")
}

/// A relative path as a URI reference: what isn't unreserved or a `/` is
/// percent-encoded.
fn uri_path(path: &str) -> String {
    let mut out = String::new();
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => out.push(byte as char),
            _ => {
                let _ = write!(out, "%{byte:02X}");
            }
        }
    }
    out
}

/// The project folder as a `file:` URI with a trailing `/`, as a base to
/// resolve the files against.
fn root_uri(root: &Path) -> String {
    let path = root.to_string_lossy().replace('\\', "/");
    let path = path.trim_end_matches('/');
    let lead = if path.starts_with('/') { "" } else { "/" };
    format!("file://{lead}{}/", uri_path(path).replace("%3A", ":"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use phoxite::base::{Pos, Range};

    fn problem(line: u32, col: u32, severity: Severity, code: Option<&str>, message: &str) -> Diagnostic {
        Diagnostic {
            range: Range::new(Pos::new(line, col), Pos::new(line, col + 4)),
            severity,
            message: message.to_owned(),
            source: Some("php".to_owned()),
            code: code.map(str::to_owned),
            unnecessary: false,
            deprecated: false,
            server: None,
        }
    }

    fn report() -> Report {
        let root = PathBuf::from(if cfg!(windows) { r"C:\work\my app" } else { "/work/my app" });
        Report {
            inspected: 7,
            files: vec![
                (
                    root.join("app").join("Models").join("User.php"),
                    vec![
                        problem(2, 0, Severity::Error, None, "Syntax error: unexpected token"),
                        problem(
                            9,
                            8,
                            Severity::Warning,
                            Some("PhpUndefinedMethod"),
                            "Method 'x' not found, 100% sure:\nsee A, B",
                        ),
                    ],
                ),
                (
                    root.join("résumé, v2.php"),
                    vec![problem(0, 6, Severity::Hint, Some("PhpUnusedAlias"), "Unused import")],
                ),
            ],
            rules: vec![("PhpUndefinedMethod".into(), Severity::Warning), ("PhpUnusedAlias".into(), Severity::Hint)],
            root,
        }
    }

    #[test]
    fn text_has_a_line_per_problem() {
        let report = report();
        assert_eq!(
            report.render(Format::Text),
            "app/Models/User.php:3:1: error [syntax] Syntax error: unexpected token\n\
             app/Models/User.php:10:9: warning [PhpUndefinedMethod] Method 'x' not found, 100% sure:\nsee A, B\n\
             résumé, v2.php:1:7: hint [PhpUnusedAlias] Unused import\n"
        );
        assert_eq!(report.summary(), "1 error, 1 warning, 1 hint in 2 of 7 files");
        let clean = Report { files: Vec::new(), inspected: 1, ..report };
        assert_eq!((clean.render(Format::Text).as_str(), clean.summary().as_str()), ("", "no problems in 1 file"));
    }

    #[test]
    fn json_has_the_problems_and_their_counts() {
        let found: Value = serde_json::from_str(&report().render(Format::Json)).unwrap();
        assert_eq!(found["files"], 7);
        assert_eq!(found["summary"], json!({ "error": 1, "warning": 1, "note": 0, "hint": 1 }));
        assert_eq!(
            found["problems"][1],
            json!({
                "file": "app/Models/User.php", "line": 10, "column": 9, "endLine": 10, "endColumn": 13,
                "severity": "warning", "code": "PhpUndefinedMethod", "source": "php",
                "message": "Method 'x' not found, 100% sure:\nsee A, B",
            })
        );
        assert_eq!(found["problems"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn github_commands_escape_what_would_end_them() {
        assert_eq!(
            report().render(Format::Github),
            "::error file=app/Models/User.php,line=3,col=1,endLine=3,endColumn=5,title=syntax::Syntax error: unexpected token\n\
             ::warning file=app/Models/User.php,line=10,col=9,endLine=10,endColumn=13,title=PhpUndefinedMethod::Method 'x' not found, 100%25 sure:%0Asee A, B\n\
             ::notice file=résumé%2C v2.php,line=1,col=7,endLine=1,endColumn=11,title=PhpUnusedAlias::Unused import\n"
        );
    }

    #[test]
    fn sarif_has_the_rules_and_a_result_per_problem() {
        let found: Value = serde_json::from_str(&report().render(Format::Sarif)).unwrap();
        assert_eq!(found["version"], "2.1.0");
        let run = &found["runs"][0];
        assert_eq!(run["tool"]["driver"]["name"], "Phoxite");
        let rules: Vec<&str> =
            run["tool"]["driver"]["rules"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
        assert_eq!(rules, ["PhpUndefinedMethod", "PhpUnusedAlias", "syntax"]);
        assert_eq!(run["tool"]["driver"]["rules"][1]["defaultConfiguration"]["level"], "note");
        let base = if cfg!(windows) { "file:///C:/work/my%20app/" } else { "file:///work/my%20app/" };
        assert_eq!(run["originalUriBaseIds"]["SRCROOT"]["uri"], base);
        let results = run["results"].as_array().unwrap();
        assert_eq!(results.len(), 3);
        assert_eq!((&results[0]["ruleId"], &results[0]["level"]), (&json!("syntax"), &json!("error")));
        assert_eq!(
            results[2]["locations"][0]["physicalLocation"],
            json!({
                "artifactLocation": { "uri": "r%C3%A9sum%C3%A9%2C%20v2.php", "uriBaseId": "SRCROOT" },
                "region": { "startLine": 1, "startColumn": 7, "endLine": 1, "endColumn": 11 },
            })
        );
        // Every result names a rule the driver declares.
        assert!(results.iter().all(|r| rules.contains(&r["ruleId"].as_str().unwrap())));
    }
}
