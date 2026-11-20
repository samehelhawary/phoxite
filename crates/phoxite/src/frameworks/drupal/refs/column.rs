//! The `drupalColumn` reference kind: a column of a database table, `nid`
//! in `db_select('node', 'n')->condition('nid', 5)`.
//!
//! The table is the one the query was made for: the type provider types
//! `db_select('node', 'n')` and the connection's `select('node', 'n')` as a
//! query of that table ([`crate::types::provider::DrupalProvider`]), which a
//! variable keeps. Its columns are the keys of the `fields` its entry of
//! `hook_schema()` has.
//!
//! A column may be written after an alias (`n.nid`): the alias of a joined
//! table (`u.uid` after `->join('users', 'u', …)`) is that table's, by the
//! calls of the function around the string. Goto goes to the
//! column in `hook_schema()`, completion offers the table's columns, and
//! Find References lists the strings naming the same column of the same
//! table. A column isn't renamed here: the database has it too.

use crate::frameworks::refs::{CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind};
use crate::navigate::Target;
use crate::summary::DeclaredKind;

/// Drupal 7's query classes and the interfaces of the later ones.
const QUERIES: &[&str] = &[
    "SelectQueryInterface",
    "SelectQuery",
    "QueryConditionInterface",
    "InsertQuery",
    "UpdateQuery",
    "DeleteQuery",
    "MergeQuery",
    "Drupal\\Core\\Database\\Query\\SelectInterface",
    "Drupal\\Core\\Database\\Query\\ConditionInterface",
    "Drupal\\Core\\Database\\Query\\Insert",
    "Drupal\\Core\\Database\\Query\\Update",
    "Drupal\\Core\\Database\\Query\\Delete",
    "Drupal\\Core\\Database\\Query\\Merge",
];

const fn query(names: &'static [&'static str], index: usize, parameter: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Method { classes: QUERIES, names }, index, parameter)
}

static PATTERNS: [PlacePattern; 7] = [
    query(&["condition", "isNull", "isNotNull", "orderBy", "groupBy", "addExpression"], 0, "field"),
    query(&["addField"], 1, "field"),
    // `fields('n', array('nid', 'title'))` of a select.
    query(&["fields"], 1, "fields").at(Position::ListValue),
    // `fields(array('title' => $title))` and `fields(array('nid', 'title'))` of an insert or an update.
    query(&["fields", "key", "keys", "insertFields", "updateFields"], 0, "fields").at(Position::ArrayKey),
    query(&["fields", "key", "keys", "insertFields", "updateFields"], 0, "fields").at(Position::ListValue),
    query(&["key"], 0, "field"),
    query(&["useDefaults"], 0, "fields").at(Position::ListValue),
];

/// See the module documentation.
pub struct Column;

/// A column as written: without the alias before it (`nid` of `n.nid`).
fn column(written: &str) -> &str {
    written.rsplit('.').next().unwrap_or(written)
}

/// The tables the function around byte `at` of `text` queries or joins, by
/// the alias it gives each: `db_select('node', 'n')`, `->join('users', 'u', …)`.
fn aliases(text: &str, at: usize) -> Vec<(String, String)> {
    const CALLS: [&str; 7] =
        ["db_select(", "select(", "join(", "innerJoin(", "leftJoin(", "rightJoin(", "leftOuterJoin("];
    let at = at.min(text.len());
    let start = text[..at].rfind("function ").unwrap_or(0);
    let end = text[at..].find("\nfunction ").map_or(text.len(), |found| at + found);
    let body = &text[start..end];
    // A string at the start of `rest`, after spaces: what it says, and what follows it.
    let string = |rest: &'_ str| -> Option<(String, usize)> {
        let trimmed = rest.trim_start();
        let quote = trimmed.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
        let close = trimmed[1..].find(quote)?;
        Some((trimmed[1..1 + close].to_owned(), rest.len() - trimmed.len() + close + 2))
    };
    let mut out = Vec::new();
    for call in CALLS {
        let mut from = 0;
        while let Some(found) = body[from..].find(call) {
            from += found + call.len();
            let Some((table, after)) = string(&body[from..]) else { continue };
            let Some(rest) = body[from + after..].trim_start().strip_prefix(',') else { continue };
            let Some((alias, _)) = string(rest) else { continue };
            if !out.iter().any(|(known, _)| *known == alias) {
                out.push((alias, table));
            }
        }
    }
    out
}

impl Column {
    /// The table of the column written `value`: the one its alias stands
    /// for in the function around the string (`u` of `u.uid`, after
    /// `->join('users', 'u', …)`), else the one the string's query was made
    /// for.
    fn table(cx: &RefContext<'_>, value: &str) -> Option<String> {
        let site = cx.site.as_ref()?;
        if let Some((alias, _)) = value.rsplit_once('.') {
            let text = match cx.engine.open_document(&site.path) {
                Some(document) => Some(document.text.to_string()),
                None => std::fs::read_to_string(&site.path).ok(),
            };
            let joined = text.and_then(|text| {
                aliases(&text, site.start as usize)
                    .into_iter()
                    .find(|(known, _)| known == alias)
                    .map(|(_, table)| table)
            });
            if joined.is_some() {
                return joined;
            }
        }
        match site.call.as_ref()? {
            (CallTarget::Method { tables, .. }, _) => tables.first().cloned(),
            _ => None,
        }
    }
}

impl ReferenceKind for Column {
    fn id(&self) -> &'static str {
        "drupalColumn"
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        &PATTERNS
    }

    /// The column with its table before it: `node.nid`.
    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        match Self::table(cx, value) {
            Some(table) => format!("{table}.{}", column(value)),
            None => value.to_owned(),
        }
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        column(value)
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(column(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<RefItem> {
        let Some(table) = Self::table(cx, value) else { return Vec::new() };
        let prefix = format!("{table}.");
        // What is written before the column stays: the alias.
        let alias = &value[..value.len() - column(value).len()];
        let names = cx.engine.names();
        let mut seen = std::collections::HashSet::new();
        names
            .of(&[DeclaredKind::DrupalColumn])
            .filter_map(|declared| declared.name.strip_prefix(&prefix))
            .filter(|name| seen.insert(name.to_owned()))
            .map(|name| RefItem {
                label: name.to_owned(),
                insert: format!("{alias}{name}"),
                detail: Some(table.clone()),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Vec<Target> {
        let Some(table) = Self::table(cx, value) else { return Vec::new() };
        let names = cx.engine.names();
        let wanted = format!("{table}.{}", column(value));
        // Not the example of an `*.api.php` file.
        let example = |path: &std::path::Path| {
            path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with(".api.php"))
        };
        names
            .named(&[DeclaredKind::DrupalColumn], &wanted)
            .into_iter()
            .filter(|declared| !example(&declared.path))
            .map(|declared| Target { path: declared.path.clone(), range: declared.range })
            .collect()
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        Self::table(cx, value).is_none() || !self.resolve(cx, value, 0).is_empty()
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        let table = Self::table(cx, value)?;
        let target = self.resolve(cx, value, 0).into_iter().next()?;
        let root = cx.engine.root();
        let file = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
        Some(format!("**column** `{}` of `{table}`\n\n{file}:{}", column(value), target.range.start.line + 1))
    }
}

#[cfg(test)]
mod tests {
    use super::aliases;

    #[test]
    fn a_functions_queries_and_joins_give_their_aliases() {
        let text = "<?php
function a() {
  $q = db_select('other', 'n');
}
function b() {
  $q = db_select('node', 'n');
  $q->join(\"users\", 'u', 'u.uid = n.uid');
  $q->leftJoin('shop_shelf', 's');
  $q->condition('u.uid', 1);
}
";
        let found = aliases(text, text.find("condition").unwrap());
        let of = |alias: &str| found.iter().find(|(known, _)| known == alias).map(|(_, table)| table.as_str());
        assert_eq!((of("n"), of("u"), of("s")), (Some("node"), Some("users"), Some("shop_shelf")));
    }
}
