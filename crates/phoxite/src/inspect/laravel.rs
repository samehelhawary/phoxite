//! The "undefined …" inspections of Laravel string references
//! ([`UndefinedReference`]), each over one reference kind and reporting
//! what [`ReferenceKind::undefined`] says, with its quick-fix:
//!
//! - `PhpUndefinedConfigKey`: a configuration key below a known array that
//!   isn't there.
//! - `PhpUndefinedEnvKey`: an `env()` key neither `.env` nor `.env.example`
//!   sets (fix: add it to `.env.example`).
//! - `PhpUndefinedTranslation`: a translation key no locale has (fix:
//!   create it in every locale).
//! - `PhpUndefinedViteAsset`: a Vite asset that is neither a file, an entry
//!   point nor in the build manifest.
//! - `PhpUnknownValidationRule`: a validation rule that is neither one of
//!   Laravel's nor registered with `Validator::extend(…)`.
//! - `PhpUnknownEloquentColumn`: a column name in a query or a model's
//!   attribute lists that the model's table doesn't have, when the
//!   migrations say for certain which columns it has.
//! - `PhpUnknownAbility`: an ability (`Gate::allows`, `$user->can`, `@can`,
//!   …) that no gate defines and that isn't a method of the policy of its
//!   model (of any policy, without a model); also the ability of `can:`
//!   middleware (`->middleware('can:update,post')`).
//! - `PhpUnknownGuard`: a guard (`auth('…')`, `Auth::guard`, `@auth`, the
//!   `auth:`/`guest:` middleware) that `config/auth.php` doesn't declare.
//! - `PhpUnknownPasswordBroker`: a password broker (`Password::broker('…')`)
//!   that `config/auth.php` doesn't declare.
//! - `PhpUndefinedInertiaPage`: an Inertia page (`Inertia::render`,
//!   `inertia()`, `Route::inertia`) no page folder has (fix: create the
//!   page).
//! - `PhpUndefinedController`: a route action naming a controller class or
//!   method that doesn't exist (fixes: create the controller, add the
//!   method; [`crate::frameworks::laravel::refs::ControllerAction`]).
//! - `PhpUnknownArtisanCommand`: an Artisan command name (`Artisan::call`,
//!   `$this->call`, `Schedule::command`, `$this->artisan`) no command has;
//!   only with `vendor/` installed, so the packages' commands are known.
//!
//! - `PhpUndefinedTemplate` (Symfony) and `PhpUndefinedHook` (WordPress,
//!   off by default) are of the other frameworks' kinds, as are
//!   CodeIgniter's eight ([`CODEIGNITER`]), Yii's four ([`YII`]), CakePHP's seven
//!   ([`CAKEPHP`]).
//!
//! They run only while the indexes are ready, each in projects of its
//! framework.

use crate::base::Severity;

use super::{Inspection, InspectionContext, Problems, QuickFix};
use crate::engine::{EngineState, is_meta_path};
use crate::frameworks::laravel::refs::{
    ArtisanCommandKind, ArtisanOptionKind, AuthGuard, ConfigKey, ControllerAction, EloquentField, EnvKey, Gate,
    InertiaPageKind, PasswordBroker, ReferenceKind, TranslationKey, ValidationRule, ViewName, ViteAsset,
};

/// One "undefined …" inspection; see the module documentation.
pub struct UndefinedReference {
    id: &'static str,
    kind: &'static dyn ReferenceKind,
}

/// One framework's inspections, as `(inspection id, kind)`.
type Inspections = &'static [(&'static str, &'static dyn ReferenceKind)];

/// The inspections of every framework's kinds: a framework with "undefined
/// …" inspections of its own adds its list here. A kind is only asked in a
/// project of its framework ([`crate::frameworks::refs::kinds_for`]), so an
/// inspection reports nothing elsewhere.
const INSPECTIONS: &[Inspections] = &[LARAVEL, SYMFONY, WORDPRESS, CODEIGNITER, YII, CAKEPHP];

/// CakePHP's: a template, a layout or an element whose file isn't there, a
/// component, a helper or a behaviour no class is of, an action its
/// controller doesn't have.
const CAKEPHP: Inspections = &[
    ("PhpUndefinedCakeTemplate", &crate::frameworks::cakephp::refs::TEMPLATE),
    ("PhpUndefinedCakeLayout", &crate::frameworks::cakephp::refs::LAYOUT),
    ("PhpUndefinedCakeElement", &crate::frameworks::cakephp::refs::ELEMENT),
    ("PhpUndefinedCakeComponent", &crate::frameworks::cakephp::refs::COMPONENT),
    ("PhpUndefinedCakeHelper", &crate::frameworks::cakephp::refs::HELPER),
    ("PhpUndefinedCakeBehavior", &crate::frameworks::cakephp::refs::BEHAVIOR),
    ("PhpUndefinedCakeAction", &crate::frameworks::cakephp::refs::ACTION),
];

/// Yii's: a view whose file isn't there, a route whose controller has no
/// such action, an attribute a form model doesn't have.
const YII: Inspections = &[
    ("PhpUndefinedYiiView", &crate::frameworks::yii::refs::VIEW),
    ("PhpUndefinedYiiLayout", &crate::frameworks::yii::refs::LAYOUT),
    ("PhpUndefinedYiiRoute", &crate::frameworks::yii::refs::ROUTE),
    ("PhpUnknownYiiAttribute", &crate::frameworks::yii::refs::ATTRIBUTE),
];

/// CodeIgniter's: a view, a helper, a language line, a route, a filter, a
/// route's handler, a validation rule, a rule group, a model, a
/// configuration, a service and a view cell that name nothing; and, turned
/// on, an event nothing fires and a table or a column no migration makes.
const CODEIGNITER: Inspections = &[
    ("PhpUndefinedCodeIgniterView", &crate::frameworks::codeigniter::refs::VIEW),
    ("PhpUndefinedCodeIgniterHelper", &crate::frameworks::codeigniter::refs::HELPER),
    ("PhpUndefinedCodeIgniterLanguageLine", &crate::frameworks::codeigniter::refs::LANG),
    ("PhpUndefinedCodeIgniterRoute", &crate::frameworks::codeigniter::refs::ROUTE),
    ("PhpUnknownCodeIgniterFilter", &crate::frameworks::codeigniter::refs::FILTER),
    ("PhpUndefinedCodeIgniterController", &crate::frameworks::codeigniter::refs::CONTROLLER),
    ("PhpUnknownCodeIgniterRule", &crate::frameworks::codeigniter::refs::RULE),
    ("PhpUnknownCodeIgniterRuleGroup", &crate::frameworks::codeigniter::refs::RULE_GROUP),
    ("PhpUndefinedCodeIgniterModel", &crate::frameworks::codeigniter::refs::MODEL),
    ("PhpUndefinedCodeIgniterConfig", &crate::frameworks::codeigniter::refs::CONFIG),
    ("PhpUndefinedCodeIgniterService", &crate::frameworks::codeigniter::refs::SERVICE),
    ("PhpUndefinedCodeIgniterCell", &crate::frameworks::codeigniter::refs::CELL),
    // Off unless turned on ([`super::OPT_IN`]).
    ("PhpUndefinedCodeIgniterEvent", &crate::frameworks::codeigniter::refs::EVENT),
    ("PhpUnknownCodeIgniterTable", &crate::frameworks::codeigniter::refs::TABLE),
    ("PhpUnknownCodeIgniterColumn", &crate::frameworks::codeigniter::refs::COLUMN),
];

/// WordPress's: `PhpUndefinedHook`, a hook listened to that nothing indexed
/// fires. Off unless turned on ([`super::OPT_IN`]).
const WORDPRESS: Inspections = &[("PhpUndefinedHook", &crate::frameworks::wordpress::refs::HOOK)];

/// Symfony's: `PhpUndefinedTemplate`, a Twig template PHP names that the
/// project's template folders don't have.
const SYMFONY: Inspections = &[("PhpUndefinedTemplate", &crate::frameworks::symfony::refs::SymfonyTemplate)];

/// Laravel's.
const LARAVEL: Inspections = &[
    ("PhpUndefinedConfigKey", &ConfigKey),
    ("PhpUndefinedEnvKey", &EnvKey),
    ("PhpUndefinedTranslation", &TranslationKey),
    ("PhpUndefinedViteAsset", &ViteAsset),
    ("PhpUnknownValidationRule", &ValidationRule),
    ("PhpUnknownEloquentColumn", &EloquentField),
    ("PhpUnknownAbility", &Gate),
    ("PhpUndefinedInertiaPage", &InertiaPageKind),
    ("PhpUnknownGuard", &AuthGuard),
    ("PhpUnknownPasswordBroker", &PasswordBroker),
    ("PhpUndefinedController", &ControllerAction),
    ("PhpUnknownArtisanCommand", &ArtisanCommandKind),
    ("PhpUnknownArtisanOption", &ArtisanOptionKind),
];

impl UndefinedReference {
    /// Every inspection of the module.
    pub fn all() -> Vec<Self> {
        INSPECTIONS.iter().flat_map(|list| list.iter()).map(|(id, kind)| Self { id, kind: *kind }).collect()
    }
}

/// The id of the inspection over the kind `kind` (`configKey`).
pub fn inspection_id(kind: &str) -> Option<&'static str> {
    // Views have their own inspection (`crate::inspect::views`).
    if kind == ViewName.id() {
        return Some("PhpUndefinedView");
    }
    INSPECTIONS.iter().flat_map(|list| list.iter()).find(|(_, known)| known.id() == kind).map(|(id, _)| *id)
}

impl Inspection for UndefinedReference {
    fn id(&self) -> &'static str {
        self.id
    }

    fn default_severity(&self) -> Severity {
        Severity::Warning
    }

    fn check(&self, cx: &InspectionContext<'_>, out: &mut Problems) {
        if cx.engine.status().state != EngineState::Ready || is_meta_path(cx.path) {
            return;
        }
        for found in cx.engine.undefined_references(cx.parsed, cx.path, &[self.kind], cx.cancel) {
            let problem = out.report(found.start, found.end, found.problem.message);
            problem.with_severity(found.problem.severity);
            for (index, fix) in found.problem.fixes.into_iter().enumerate() {
                problem.with_fix(QuickFix { title: fix.title, edit: fix.edit, preferred: index == 0, open: fix.open });
            }
        }
    }
}
