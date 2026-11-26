//! The shared machinery itself: what every framework's kinds rely on, whichever
//! framework they are of. Each framework's own names are tested with it
//! (`navigate/tests/<framework>.rs`).

use std::collections::HashMap;

use super::*;
use crate::frameworks::Framework;

/// A kind is asked for by its id (`kind_named`, an inspection's kind, what a template
/// asks the project): two kinds with one id would answer for each other.
#[test]
fn every_kind_has_an_id_of_its_own() {
    let mut owners: HashMap<&str, Framework> = HashMap::new();
    for framework in Framework::ALL {
        for kind in framework_kinds(framework) {
            let id = kind.id();
            assert!(!id.is_empty(), "a kind of {} has no id", framework.title());
            if let Some(other) = owners.insert(id, framework) {
                panic!("`{id}` is a kind of {} and of {}", other.title(), framework.title());
            }
        }
    }
}

/// Each framework has kinds, and a project's kinds are those of its frameworks, in
/// their order: a kind is never asked in a project of another framework.
#[test]
fn a_projects_kinds_are_those_of_its_frameworks() {
    assert!(kinds_for(Frameworks::default()).is_empty());
    for framework in Framework::ALL {
        assert!(!framework_kinds(framework).is_empty(), "{} has no kinds", framework.title());
        let mut one = Frameworks::default();
        one.insert(framework);
        let ids = |kinds: &[&'static dyn ReferenceKind]| kinds.iter().map(|kind| kind.id()).collect::<Vec<_>>();
        assert_eq!(ids(&kinds_for(one)), ids(framework_kinds(framework)));
    }
    let mut two = Frameworks::default();
    two.insert(Framework::Yii);
    two.insert(Framework::Laravel);
    let wanted = framework_kinds(Framework::Laravel).len() + framework_kinds(Framework::Yii).len();
    assert_eq!(kinds_for(two).len(), wanted);
}

#[test]
fn a_kind_is_found_by_its_id_among_the_kinds_given() {
    let laravel = framework_kinds(Framework::Laravel);
    let first = laravel[0].id();
    assert_eq!(kind_named(laravel, first).map(|kind| kind.id()), Some(first));
    assert!(kind_named(laravel, "noSuchKind").is_none());
    // Not among another framework's.
    assert!(kind_named(framework_kinds(Framework::Yii), first).is_none());
}

/// The literal around an offset: where its quote is, where its content ends, and
/// whether it is closed (a string being typed often isn't).
#[test]
fn the_literal_around_an_offset_is_found_closed_or_not() {
    let text = "<?php route('home.index');";
    let quote = text.find('\'').unwrap();
    let close = text.rfind('\'').unwrap();
    assert_eq!(literal_around(text, quote + 3), Some((quote, close, true)));
    // At the very start of the content, and at its end.
    assert_eq!(literal_around(text, quote + 1), Some((quote, close, true)));
    assert_eq!(literal_around(text, close), Some((quote, close, true)));

    // Not closed: the content ends with the line, without the spaces after it.
    let open = "<?php route('home.in  \n$next = 1;";
    let quote = open.find('\'').unwrap();
    let typed = open.find("  \n").unwrap();
    assert_eq!(literal_around(open, typed), Some((quote, typed, false)));

    // An escaped quote doesn't end it.
    let escaped = "<?php trans('it\\'s here');";
    let quote = escaped.find('\'').unwrap();
    let close = escaped.rfind('\'').unwrap();
    assert_eq!(literal_around(escaped, quote + 2), Some((quote, close, true)));

    // Outside a string there is none.
    assert_eq!(literal_around(text, 3), None);
}
