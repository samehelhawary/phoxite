//! `docs/inspections.md` is written from the inspections themselves, and this test fails
//! when it no longer says what they do: an inspection added, renamed or with another
//! default. `PHOXITE_WRITE_DOCS=1 cargo test -p phoxite --lib inspection_reference` writes
//! it again.

use std::path::Path;

use super::*;

const INTRO: &str = "\
# Inspections

Every inspection the engine has, by the id the `inspections` setting takes (`phoxite.inspections`
in VS Code, `inspections` in `phoxite.json`) and a `@noinspection` comment names. Each can be
turned off or on (`enabled`) and given another `severity`; the ones marked *strict* also take
`\"strict\": true`, which reports what is only probably wrong.

```json
{ \"inspections\": { \"PhpUnusedParameter\": { \"enabled\": false }, \"PhpUndefinedHook\": { \"enabled\": true } } }
```

A framework's inspections report in projects of that framework only. This page is written
by a test (`crates/phoxite/src/inspect/reference_tests.rs`): don't edit the table by hand.

| Inspection | Severity | On by default | Strict mode |
| --- | --- | --- | --- |
";

fn reference() -> String {
    // The mode a strict inspection is asked for tells it from the others.
    let strict = parking_lot::Mutex::new(Vec::new());
    let all = Inspections::builtin_modes(&|id| {
        strict.lock().push(id);
        TypeCheckMode::default()
    });
    let strict = strict.into_inner();
    let mut rows: Vec<(&str, &str, bool, bool)> = all
        .iter()
        .map(|inspection| {
            let id = inspection.id();
            let severity = match inspection.default_severity() {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "information",
                Severity::Hint => "hint",
            };
            (id, severity, !OPT_IN.contains(&id), strict.contains(&id))
        })
        .collect();
    rows.sort();
    rows.dedup();
    let mut text = INTRO.to_owned();
    for (id, severity, on, strict) in rows {
        let (on, strict) = (if on { "yes" } else { "no" }, if strict { "yes" } else { "" });
        text.push_str(&format!("| `{id}` | {severity} | {on} | {strict} |\n"));
    }
    text
}

#[test]
fn the_inspection_reference_says_what_the_inspections_do() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/inspections.md");
    let wanted = reference();
    if std::env::var_os("PHOXITE_WRITE_DOCS").is_some() {
        std::fs::write(&path, &wanted).unwrap();
    }
    let written = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
    assert!(written == wanted, "docs/inspections.md is out of date: run this test with PHOXITE_WRITE_DOCS=1");
}
