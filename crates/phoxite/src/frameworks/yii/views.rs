//! What a Yii view is given: `$this->render('index', ['post' => $post])`
//! makes `$post` a variable of `views/site/index.php`, of the type of what
//! is passed. A mail view has what `compose('welcome', ['user' => $user])`
//! passes. `$this` in a view is the view object (`yii\web\View`).
//!
//! A view is a plain PHP file whose variables nothing in it sets. The
//! types come to the file's own inference through
//! [`Declarations::view_variable`](crate::types::Declarations::view_variable):
//! hover, completion and the inspections of the view see them. A view's
//! own `/* @var $post app\models\Post */` still says more than a call does.

use std::path::Path;
use std::sync::Arc;

use crate::engine::PhpEngine;
use crate::frameworks::Framework;
use crate::frameworks::refs::{ReferenceKind, sites};
use crate::frameworks::yii::refs::{MAIL, VIEW};
use crate::names::FileNames;
use crate::summary::FileSummary;
use crate::types::{ExprTypes, Type};

impl PhpEngine {
    /// Whether the file at `path` is a view of a Yii project: a PHP file of
    /// the project below a `views` or a `mail` folder, or where the
    /// configuration puts views (a `viewPath`, a theme).
    pub fn is_yii_view(&self, path: &Path) -> bool {
        self.frameworks().contains(Framework::Yii)
            && path.extension().is_some_and(|extension| extension == "php")
            && self.yii().is_view(path)
            && !self.is_library(path)
    }

    /// The variables the project's code passes the view at `path`:
    /// `(name, type)`, sorted by name, the types of one name joined.
    pub fn yii_view_variables(&self, path: &Path) -> Arc<Vec<(String, Type)>> {
        if !self.is_yii_view(path) {
            return Arc::default();
        }
        let count = self.edit_count();
        if let Some((at, known)) = self.yii_views_cache().lock().get(path)
            && *at == count
        {
            return known.clone();
        }
        // While this view is worked out, a view asking again has none (a view rendering itself).
        let Some(_working) = self.view_in_progress(path) else { return Arc::default() };
        // Every usage names the view by its file ([`ReferenceKind::canonical`]).
        let root = self.root();
        let key = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/");
        let open: Vec<_> = self.open_documents().into_iter().filter(|document| !document.embedded).collect();
        let resolver = self.resolver();
        let mut passed: Vec<(String, Type)> = Vec::new();
        let kinds: [&'static dyn ReferenceKind; 2] = [&VIEW, &MAIL];
        for kind in kinds {
            let text = kind.usage_text(&key);
            for candidate in self.usage_candidates(kind, &key).unwrap_or_default() {
                let Some(file) = self.usage_file(&candidate, &open, text) else { continue };
                let starts: Vec<u32> = sites(&file.parsed, &resolver, &[kind], None)
                    .into_iter()
                    .filter(|site| {
                        kind.patterns().iter().any(|pattern| pattern.matches(site, &resolver))
                            && self
                                .site_context(&candidate, site)
                                .is_some_and(|cx| kind.canonical(&cx, &site.value) == key)
                    })
                    .map(|site| site.start)
                    .collect();
                if starts.is_empty() {
                    continue;
                }
                let names = FileNames::resolve(&file.parsed);
                let summary = FileSummary::extract(&file.parsed);
                let types = ExprTypes::new(&file.parsed, &names, &summary, &resolver).at_path(Some(&candidate));
                passed.extend(crate::frameworks::laravel::views::passed_variables(&types, &file.parsed, &starts));
            }
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
        self.yii_views_cache().lock().insert(path.to_path_buf(), (count, variables.clone()));
        variables
    }
}
