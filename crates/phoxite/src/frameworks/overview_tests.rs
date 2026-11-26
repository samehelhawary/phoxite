//! `docs/frameworks.md` is written from the frameworks themselves, and this test fails
//! when it no longer says what they have: a framework, a kind of name or an inspection
//! added or renamed. `PHOXITE_WRITE_DOCS=1 cargo test -p phoxite --lib framework_overview`
//! writes it again.

use std::path::Path;

use super::Framework;
use super::refs::framework_kinds;
use crate::inspect::references::inspection_id;

const INTRO: &str = "\
# Frameworks

What each framework's support knows, side by side. A framework is found by a Composer
package or one of its own files (`docs/engine/frameworks.md` has the files), and the
`frameworks` setting turns each on or off (`docs/settings.md`).

The *names* are the kinds of string the support reads: in a string of that kind, completion
lists what the project declares, go to definition and hover find it, and its usages are
found and, for most, renamed with it. The *inspections* report such a name that names
nothing (`docs/inspections.md`). The types each framework adds (services, models, view
variables) and what it reads besides PHP are in its note in `docs/engine`.

This page is written by a test (`crates/phoxite/src/frameworks/overview_tests.rs`): don't
edit it by hand.

| Framework | Setting | Composer packages | Kinds of name | Inspections |
| --- | --- | --- | --- | --- |
";

fn overview() -> String {
    let mut text = INTRO.to_owned();
    let mut details = String::new();
    for framework in Framework::ALL {
        let kinds = framework_kinds(framework);
        let mut inspections: Vec<&str> = kinds.iter().filter_map(|kind| inspection_id(kind.id())).collect();
        // The inspections that aren't over one kind (`inspect::routes`, `inspect::magento`).
        match framework {
            Framework::Laravel | Framework::Symfony => inspections.push("PhpUndefinedRoute"),
            Framework::Magento => inspections.push("PhpUndefinedMagentoPluginMethod"),
            _ => {}
        }
        inspections.sort();
        inspections.dedup();
        let packages: Vec<String> = framework.packages().iter().map(|package| format!("`{package}`")).collect();
        let packages = if packages.is_empty() { "none".to_owned() } else { packages.join(", ") };
        let (title, key) = (framework.title(), framework.key());
        text.push_str(&format!(
            "| [{title}](#{anchor}) | `frameworks.{key}` | {packages} | {} | {} |\n",
            kinds.len(),
            inspections.len(),
            anchor = title.to_ascii_lowercase(),
        ));
        let names: Vec<String> = kinds.iter().map(|kind| format!("`{}`", kind.id())).collect();
        let reported: Vec<String> = inspections.iter().map(|id| format!("`{id}`")).collect();
        let reported = if reported.is_empty() { "none".to_owned() } else { reported.join(", ") };
        details.push_str(&format!(
            "\n## {title}\n\nNames: {}.\n\nInspections: {reported}.\n\nMore: [engine/{key}.md](engine/{key}.md).\n",
            names.join(", ")
        ));
    }
    text + &details
}

#[test]
fn the_framework_overview_says_what_the_frameworks_have() {
    let docs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let wanted = overview();
    if std::env::var_os("PHOXITE_WRITE_DOCS").is_some() {
        std::fs::write(docs.join("frameworks.md"), &wanted).unwrap();
    }
    let written = std::fs::read_to_string(docs.join("frameworks.md")).unwrap_or_default().replace("\r\n", "\n");
    assert!(written == wanted, "docs/frameworks.md is out of date: run this test with PHOXITE_WRITE_DOCS=1");
    // Each framework's note is where the page says it is.
    for framework in Framework::ALL {
        let note = docs.join("engine").join(format!("{}.md", framework.key()));
        assert!(note.is_file(), "{} is missing", note.display());
    }
}
