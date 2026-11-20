//! What a CodeIgniter view is given: `view('books/list', ['books' =>
//! $books])` makes `$books` a variable of `app/Views/books/list.php`, of
//! the type of what is passed.
//!
//! A layout a view extends (`$this->extend('layout')`) and a view it
//! includes (`$this->include('books/row')`) have what that view was passed.
//!
//! A view is a plain PHP file whose variables nothing in it sets. The
//! types come to the file's own inference through
//! [`Declarations::view_variable`](crate::types::Declarations::view_variable):
//! hover, completion and the inspections of the view see them.

use std::path::Path;
use std::sync::Arc;

use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::codeigniter::refs::VIEW;
use crate::frameworks::refs::{ReferenceKind, sites};
use crate::names::FileNames;
use crate::summary::FileSummary;
use crate::types::{ExprTypes, Type};

impl PhpEngine {
    /// The name of the view the file at `path` is: its path below
    /// `app/Views` without `.php` (`books/list`). `None` for another file,
    /// and outside a CodeIgniter project.
    pub fn codeigniter_view_name(&self, path: &Path) -> Option<String> {
        if !self.frameworks().contains(Framework::CodeIgniter) {
            return None;
        }
        if path.extension().is_none_or(|extension| extension != "php") {
            return None;
        }
        self.codeigniter().view_name(path)
    }

    /// The variables the project's code passes the view at `path`:
    /// `(name, type)`, sorted by name, the types of one name joined.
    pub fn codeigniter_view_variables(&self, path: &Path) -> Arc<Vec<(String, Type)>> {
        let Some(name) = self.codeigniter_view_name(path) else { return Arc::default() };
        let count = self.edit_count();
        if let Some((at, known)) = self.codeigniter_views_cache().lock().get(path)
            && *at == count
        {
            return known.clone();
        }
        // While this view is worked out, a view asking again has none (a view rendering itself).
        self.codeigniter_views_cache().lock().insert(path.to_path_buf(), (count, Arc::default()));
        let kind: &'static dyn ReferenceKind = &VIEW;
        let candidates = self.usage_candidates(kind, &name).unwrap_or_default();
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let resolver = self.resolver();
        let mut passed: Vec<(String, Type)> = Vec::new();
        for candidate in candidates {
            if candidate == path {
                continue;
            }
            let Some(file) = self.usage_file(&candidate, &open, &name) else { continue };
            let starts: Vec<u32> = sites(&file.parsed, &resolver, &[kind], None)
                .into_iter()
                .filter(|site| {
                    kind.refers_to(&site.value, &name)
                        && kind.patterns().iter().any(|pattern| pattern.matches(site, &resolver))
                })
                .map(|site| site.start)
                .collect();
            if starts.is_empty() {
                continue;
            }
            // A view extending or including this one shares what it was passed.
            if self.codeigniter_view_name(&candidate).is_some() {
                passed.extend(self.codeigniter_view_variables(&candidate).iter().cloned());
            }
            let names = FileNames::resolve(&file.parsed);
            let summary = FileSummary::extract(&file.parsed);
            let types = ExprTypes::new(&file.parsed, &names, &summary, &resolver).at_path(Some(&candidate));
            passed.extend(crate::frameworks::laravel::views::passed_variables(&types, &file.parsed, &starts));
        }
        let mut names: Vec<String> = passed.iter().map(|(name, _)| name.clone()).collect();
        names.sort();
        names.dedup();
        let variables: Vec<(String, Type)> = names
            .into_iter()
            .map(|name| {
                let types: Vec<Type> =
                    passed.iter().filter(|(known, _)| *known == name).map(|(_, ty)| ty.clone()).collect();
                (name, Type::union(types))
            })
            .collect();
        let variables = Arc::new(variables);
        self.codeigniter_views_cache().lock().insert(path.to_path_buf(), (count, variables.clone()));
        variables
    }
}
