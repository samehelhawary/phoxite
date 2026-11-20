//! CodeIgniter 4's reference kinds ([`kinds`]), on the machinery every
//! framework shares ([`crate::frameworks::refs`]). All are one type,
//! [`CodeIgniterName`].
//!
//! | kind | names | where |
//! | --- | --- | --- |
//! | `codeigniterView` | a file of `app/Views` (`books/list` is `books/list.php`) | `view()`, and a view's `$this->extend()`, `$this->include()` |
//! | `codeigniterHelper` | `<name>_helper.php` of `app/Helpers` or the framework's | `helper()` (one or a list), a controller's `$helpers` |
//! | `codeigniterLang` | a line of a language file: `File.key` | `lang()` |
//! | `codeigniterRoute` | the `as` of a route's options | `route_to()`, `url_to()`, `redirect()`, `redirect()->route()` |
//! | `codeigniterFilter` | a key of `Config\Filters::$aliases` | `'filter' => '…'` of a route's options, the lists of the filters configuration |
//! | `codeigniterConfig` | the class `Config\<Name>` | `config()` |
//! | `codeigniterModel` | the class `App\Models\<Name>` (or the class written) | `model()` |
//! | `codeigniterService` | a static method of `Config\Services` (or the framework's) | `service()`, `single_service()` |
//! | `codeigniterController` | a method of a controller: `Books::show/$1`, in the namespace of the group around it (`'namespace' => …`), else the default one | the handler of a route (`$routes->get('books/(:num)', 'Books::show/$1')`) |
//! | `codeigniterCell` | a method of a class of `App\Cells`: `Card::render` | `view_cell()` |
//! | `codeigniterEvent` | what `Events::trigger('name')` fires | `Events::on()`, `trigger()`, `removeListener()` |
//! | `codeigniterRule` | a method of a class of `Config\Validation::$ruleSets`, each rule of `required|min_length[3]` | `setRules()`, `setRule()`, a controller's `validate()` and `validateData()`, a model's `$validationRules`, the groups of `Config\Validation` |
//! | `codeigniterRuleGroup` | a property of `Config\Validation` | `run($data, 'group')`, `setRuleGroup()`, a controller's `validate('group')` |
//! | `codeigniterTable` | a table a migration makes (`createTable('books')`) | the connection's `table()`, a model's `$table`, `from()`, `join()`, the forge's methods |
//! | `codeigniterColumn` | a field of that table (`addField()`, `addColumn()`) | a model's `$allowedFields`, `$primaryKey` and the like; `where()`, `orderBy()`, `select()`… of a model or of a builder of a known table |
//!
//! A view of a module is named with its namespace (`Blog\Views\list`), and
//! a route by its `as`, by its path when it has none, or by its handler
//! (`route_to('Books::show')`).
//!
//! Each has completion, goto, hover and Find References. A route name, a
//! filter alias and an event are renamed: where they are declared and
//! wherever a string names them. A view, a helper and a language line
//! aren't: their names are files' or are shared with every locale's file.
//!
//! A view, a helper, a language line, a route, a filter, a route's handler,
//! a rule and a rule group that name nothing are reported
//! ([`ReferenceKind::undefined`]), where the project says enough to know.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::base::Range;
use crate::frameworks::codeigniter::CodeIgniterProject;
use crate::frameworks::codeigniter::project::Def;
use crate::frameworks::laravel::refs::controller_action::method_target;
use crate::frameworks::refs::{
    CallTarget, CalleePattern, PlacePattern, Position, RefContext, RefItem, ReferenceKind, RenameTarget, Undefined,
};
use crate::navigate::Target;
use crate::summary::DeclaredKind;
use crate::summary::FileSummary;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    View,
    Helper,
    Lang,
    Route,
    Filter,
    Config,
    Model,
    Service,
    Controller,
    Cell,
    Event,
    Rule,
    RuleGroup,
    Table,
    Column,
    Library,
    ConfigItem,
    Uri,
}

/// One of the kinds of the module documentation.
pub struct CodeIgniterName {
    id: &'static str,
    label: &'static str,
    source: Source,
    patterns: &'static [PlacePattern],
}

const fn function(name: &'static str, index: usize, parameter: &'static str) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Function(name), index, parameter)
}

const fn method(
    classes: &'static [&'static str],
    names: &'static [&'static str],
    index: usize,
    name: &'static str,
) -> PlacePattern {
    PlacePattern::argument(CalleePattern::Method { classes, names }, index, name)
}

/// CodeIgniter 3's loader: `$this->load`.
const LOADER: &[&str] = &["CI_Loader"];

pub static LIBRARY: CodeIgniterName = CodeIgniterName {
    id: "codeigniterLibrary",
    label: "library",
    source: Source::Library,
    patterns: &[
        method(LOADER, &["library"], 0, "library"),
        method(LOADER, &["library"], 0, "library").at(Position::ListValue),
    ],
};

pub static CONFIG_ITEM: CodeIgniterName = CodeIgniterName {
    id: "codeigniterConfigItem",
    label: "configuration item",
    source: Source::ConfigItem,
    patterns: &[
        method(&["CI_Config"], &["item", "slash_item", "set_item"], 0, "item"),
        function("config_item", 0, "item"),
    ],
};

pub static URI: CodeIgniterName = CodeIgniterName {
    id: "codeigniterUri",
    label: "controller",
    source: Source::Uri,
    patterns: &[
        function("site_url", 0, "uri"),
        function("anchor", 0, "uri"),
        function("form_open", 0, "action"),
        // What a route of `config/routes.php` is sent to.
        PlacePattern::argument(CalleePattern::Entry("$route"), 0, "route"),
        method(&["CodeIgniter\\HTTP\\RedirectResponse"], &["to"], 0, "uri"),
    ],
};

const VIEW_CLASSES: &[&str] =
    &[crate::frameworks::refs::TEMPLATE_THIS, "CodeIgniter\\View\\View", "CodeIgniter\\View\\RendererInterface"];
const FILTERS: &[&str] = &["Config\\Filters", "CodeIgniter\\Config\\Filters"];
const SERVICES: [&str; 2] = ["Config\\Services", "CodeIgniter\\Config\\Services"];

pub static VIEW: CodeIgniterName = CodeIgniterName {
    id: "codeigniterView",
    label: "view",
    source: Source::View,
    patterns: &[
        function("view", 0, "name"),
        method(VIEW_CLASSES, &["extend", "include", "render"], 0, "view"),
        method(LOADER, &["view"], 0, "view"),
    ],
};

pub static HELPER: CodeIgniterName = CodeIgniterName {
    id: "codeigniterHelper",
    label: "helper",
    source: Source::Helper,
    patterns: &[
        function("helper", 0, "filenames"),
        function("helper", 0, "filenames").at(Position::ListValue),
        method(LOADER, &["helper", "helpers"], 0, "helpers"),
        method(LOADER, &["helper", "helpers"], 0, "helpers").at(Position::ListValue),
        PlacePattern::argument(
            CalleePattern::Property { classes: &["CodeIgniter\\Controller"], names: &["helpers"] },
            0,
            "helpers",
        )
        .at(Position::ListValue),
    ],
};

pub static LANG: CodeIgniterName = CodeIgniterName {
    id: "codeigniterLang",
    label: "language line",
    source: Source::Lang,
    patterns: &[function("lang", 0, "line"), method(&["CI_Lang"], &["line"], 0, "line")],
};

pub static ROUTE: CodeIgniterName = CodeIgniterName {
    id: "codeigniterRoute",
    label: "route",
    source: Source::Route,
    patterns: &[
        function("route_to", 0, "method"),
        function("url_to", 0, "controller"),
        function("redirect", 0, "route"),
        method(&["CodeIgniter\\HTTP\\RedirectResponse"], &["route"], 0, "route"),
        // Where it is declared: the options of a route.
        PlacePattern::argument(CalleePattern::Entry("as"), 0, "as"),
    ],
};

pub static FILTER: CodeIgniterName = CodeIgniterName {
    id: "codeigniterFilter",
    label: "filter",
    source: Source::Filter,
    patterns: &[
        PlacePattern::argument(CalleePattern::Entry("filter"), 0, "filter"),
        PlacePattern::argument(CalleePattern::Entry("filter"), 0, "filter").at(Position::ListValue),
        // The configuration's own lists: `$globals`, `$methods`, `$required`, and the keys of `$filters`.
        PlacePattern::argument(
            CalleePattern::Property { classes: FILTERS, names: &["globals", "methods", "required"] },
            0,
            "filters",
        )
        .at(Position::ListValue)
        .nested(),
        PlacePattern::argument(CalleePattern::Property { classes: FILTERS, names: &["filters"] }, 0, "filters")
            .at(Position::ArrayKey),
    ],
};

pub static CONFIG: CodeIgniterName = CodeIgniterName {
    id: "codeigniterConfig",
    label: "configuration",
    source: Source::Config,
    patterns: &[function("config", 0, "name")],
};

pub static MODEL: CodeIgniterName = CodeIgniterName {
    id: "codeigniterModel",
    label: "model",
    source: Source::Model,
    patterns: &[
        function("model", 0, "name"),
        method(LOADER, &["model"], 0, "model"),
        method(LOADER, &["model"], 0, "model").at(Position::ListValue),
    ],
};

pub static SERVICE: CodeIgniterName = CodeIgniterName {
    id: "codeigniterService",
    label: "service",
    source: Source::Service,
    patterns: &[function("service", 0, "name"), function("single_service", 0, "name")],
};

const ROUTES: &[&str] = &["CodeIgniter\\Router\\RouteCollectionInterface", "CodeIgniter\\Router\\RouteCollection"];

pub static CONTROLLER: CodeIgniterName = CodeIgniterName {
    id: "codeigniterController",
    label: "controller",
    source: Source::Controller,
    patterns: &[method(
        ROUTES,
        &["get", "post", "put", "patch", "delete", "options", "head", "add", "cli", "match"],
        1,
        "to",
    )],
};

pub static CELL: CodeIgniterName = CodeIgniterName {
    id: "codeigniterCell",
    label: "view cell",
    source: Source::Cell,
    patterns: &[function("view_cell", 0, "library")],
};

pub static EVENT: CodeIgniterName = CodeIgniterName {
    id: "codeigniterEvent",
    label: "event",
    source: Source::Event,
    patterns: &[method(
        &["CodeIgniter\\Events\\Events"],
        &["on", "trigger", "removeListener", "removeAllListeners", "listeners"],
        0,
        "eventName",
    )],
};

const VALIDATION: &[&str] = &["CodeIgniter\\Validation\\ValidationInterface", "CodeIgniter\\Validation\\Validation"];
const CONTROLLERS: &[&str] = &["CodeIgniter\\Controller"];
const MODELS: &[&str] = &["CodeIgniter\\BaseModel", "CodeIgniter\\Model"];
const VALIDATION_CONFIG: &[&str] = &["Config\\Validation"];

/// The three ways rules are written for a field: a string, the `rules` of
/// an array describing the field, and a list of rules (in either).
const fn rules(callee: CalleePattern, index: usize, name: &'static str) -> [PlacePattern; 3] {
    [
        PlacePattern::argument(callee, index, name).at(Position::ArrayValue),
        PlacePattern::argument(callee, index, name).at(Position::ValueOfKey("rules")).nested(),
        PlacePattern::argument(callee, index, name).at(Position::ListValue).nested(),
    ]
}

const SET_RULES: [PlacePattern; 3] =
    rules(CalleePattern::Method { classes: VALIDATION, names: &["setRules"] }, 0, "rules");
const VALIDATE: [PlacePattern; 3] =
    rules(CalleePattern::Method { classes: CONTROLLERS, names: &["validate"] }, 0, "rules");
const VALIDATE_DATA: [PlacePattern; 3] =
    rules(CalleePattern::Method { classes: CONTROLLERS, names: &["validateData"] }, 1, "rules");
const MODEL_RULES: [PlacePattern; 3] =
    rules(CalleePattern::Property { classes: MODELS, names: &["validationRules"] }, 0, "rules");
/// Every property of the validation configuration: a group of rules.
const GROUP_RULES: [PlacePattern; 3] =
    rules(CalleePattern::Property { classes: VALIDATION_CONFIG, names: &["*"] }, 0, "rules");

pub static RULE: CodeIgniterName = CodeIgniterName {
    id: "codeigniterRule",
    label: "validation rule",
    source: Source::Rule,
    patterns: &[
        SET_RULES[0],
        SET_RULES[1],
        SET_RULES[2],
        VALIDATE[0],
        VALIDATE[1],
        VALIDATE[2],
        VALIDATE_DATA[0],
        VALIDATE_DATA[1],
        VALIDATE_DATA[2],
        MODEL_RULES[0],
        MODEL_RULES[1],
        MODEL_RULES[2],
        GROUP_RULES[0],
        GROUP_RULES[1],
        GROUP_RULES[2],
        method(VALIDATION, &["setRule"], 2, "rules"),
        method(VALIDATION, &["setRule"], 2, "rules").at(Position::ListValue),
    ],
};

pub static RULE_GROUP: CodeIgniterName = CodeIgniterName {
    id: "codeigniterRuleGroup",
    label: "rule group",
    source: Source::RuleGroup,
    patterns: &[
        method(VALIDATION, &["run"], 1, "group"),
        method(VALIDATION, &["setRuleGroup", "getRuleGroup", "loadRuleGroup"], 0, "group"),
        method(CONTROLLERS, &["validate"], 0, "rules"),
        method(CONTROLLERS, &["validateData"], 1, "rules"),
    ],
};

const CONNECTIONS: &[&str] = &["CodeIgniter\\Database\\ConnectionInterface", "CodeIgniter\\Database\\BaseConnection"];
const FORGE: &[&str] = &["CodeIgniter\\Database\\Forge"];
/// What is asked for rows: a builder, and a model (which hands on to its own).
const QUERIES: &[&str] = &["CodeIgniter\\Database\\BaseBuilder", "CodeIgniter\\BaseModel", "CodeIgniter\\Model"];

pub static TABLE: CodeIgniterName = CodeIgniterName {
    id: "codeigniterTable",
    label: "table",
    source: Source::Table,
    patterns: &[
        method(CONNECTIONS, &["table", "tableExists", "getFieldNames", "getFieldData"], 0, "tableName"),
        method(QUERIES, &["from", "join"], 0, "table"),
        method(
            FORGE,
            &["createTable", "dropTable", "renameTable", "addColumn", "dropColumn", "modifyColumn"],
            0,
            "table",
        ),
        method(FORGE, &["addForeignKey"], 1, "tableName"),
        PlacePattern::argument(CalleePattern::Property { classes: MODELS, names: &["table"] }, 0, "table"),
        method(LEGACY_DB, LEGACY_TABLES, 0, "table"),
        method(
            LEGACY_FORGE,
            &["create_table", "drop_table", "add_column", "drop_column", "modify_column", "rename_table"],
            0,
            "table",
        ),
    ],
};

/// CodeIgniter 3's query builder (`$this->db`) and its forge.
const LEGACY_DB: &[&str] = &["CI_DB_query_builder", "CI_DB_driver", "CI_DB"];
const LEGACY_FORGE: &[&str] = &["CI_DB_forge"];
/// Its methods that take a table first.
const LEGACY_TABLES: &[&str] = &[
    "get",
    "get_where",
    "from",
    "join",
    "insert",
    "insert_batch",
    "update",
    "update_batch",
    "replace",
    "delete",
    "truncate",
    "empty_table",
    "count_all",
    "count_all_results",
    "table_exists",
    "list_fields",
];
const LEGACY_FILTERING: &[&str] = &[
    "where",
    "or_where",
    "where_in",
    "or_where_in",
    "where_not_in",
    "or_where_not_in",
    "like",
    "or_like",
    "not_like",
    "or_not_like",
    "having",
    "or_having",
    "order_by",
    "group_by",
    "select",
    "select_max",
    "select_min",
    "select_avg",
    "select_sum",
    "set",
];

const FILTERING: &[&str] = &[
    "where",
    "orWhere",
    "whereIn",
    "orWhereIn",
    "whereNotIn",
    "orWhereNotIn",
    "like",
    "orLike",
    "notLike",
    "orNotLike",
    "having",
    "orHaving",
    "orderBy",
    "groupBy",
    "select",
    "selectMax",
    "selectMin",
    "selectAvg",
    "selectSum",
    "selectCount",
    "set",
    "findColumn",
    "increment",
    "decrement",
];
const KEYED: &[&str] = &["where", "orWhere", "like", "orLike", "having", "set", "insert", "save", "replace", "update"];

pub static COLUMN: CodeIgniterName = CodeIgniterName {
    id: "codeigniterColumn",
    label: "column",
    source: Source::Column,
    patterns: &[
        method(QUERIES, FILTERING, 0, "key"),
        method(QUERIES, KEYED, 0, "key").at(Position::ArrayKey),
        method(MODELS, &["update"], 1, "row").at(Position::ArrayKey),
        method(LEGACY_DB, LEGACY_FILTERING, 0, "key"),
        method(LEGACY_DB, &["where", "or_where", "like", "or_like", "having", "set"], 0, "key").at(Position::ArrayKey),
        method(LEGACY_DB, &["insert", "update", "replace"], 1, "set").at(Position::ArrayKey),
        method(LEGACY_FORGE, &["drop_column"], 1, "column_name"),
        method(FORGE, &["dropColumn"], 1, "columnNames"),
        method(FORGE, &["dropColumn"], 1, "columnNames").at(Position::ListValue),
        PlacePattern::argument(CalleePattern::Property { classes: MODELS, names: &["allowedFields"] }, 0, "fields")
            .at(Position::ListValue),
        PlacePattern::argument(
            CalleePattern::Property {
                classes: MODELS,
                names: &["primaryKey", "createdField", "updatedField", "deletedField"],
            },
            0,
            "field",
        ),
        PlacePattern::argument(
            CalleePattern::Property { classes: MODELS, names: &["validationRules", "casts"] },
            0,
            "fields",
        )
        .at(Position::ArrayKey),
    ],
};

/// CodeIgniter's kinds.
pub fn kinds() -> &'static [&'static dyn ReferenceKind] {
    static KINDS: [&dyn ReferenceKind; 18] = [
        &VIEW,
        &HELPER,
        &LANG,
        &ROUTE,
        &FILTER,
        &CONFIG,
        &MODEL,
        &SERVICE,
        &CONTROLLER,
        &CELL,
        &EVENT,
        &RULE,
        &RULE_GROUP,
        &TABLE,
        &COLUMN,
        &LIBRARY,
        &CONFIG_ITEM,
        &URI,
    ];
    &KINDS
}

/// From where to where in a string.
type Span = (usize, usize);

/// One rule of a rule string: its name and where it is in the string.
pub(crate) struct RuleSegment<'v> {
    pub name: &'v str,
    pub start: usize,
    pub name_end: usize,
    pub end: usize,
}

/// The rules of `required|min_length[3]|in_list[a,b]`: split at the `|`
/// outside brackets.
pub(crate) fn rule_segments(value: &str) -> Vec<RuleSegment<'_>> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut push = |start: usize, end: usize| {
        let text = &value[start..end];
        let name_end = start + text.find('[').unwrap_or(text.len());
        out.push(RuleSegment { name: &value[start..name_end], start, name_end, end });
    };
    for (at, c) in value.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            '|' if depth == 0 => {
                push(start, at);
                start = at + 1;
            }
            _ => {}
        }
    }
    push(start, value.len());
    out
}

/// Rules the validation runs itself, which no rule set declares.
const BUILT_IN_RULES: [&str; 2] = ["if_exist", "permit_empty"];

/// The class a name of `config()` or `model()` stands for: the one
/// written (`App\Models\BookModel`), else `<namespace>\<Name>`.
pub(crate) fn class_of(namespace: &str, name: &str) -> String {
    let name = name.trim_start_matches('\\');
    if name.contains('\\') { name.to_owned() } else { format!("{namespace}\\{name}") }
}

/// The files below `folder` with extension `php`, as paths relative to it
/// without the extension (`books/list`), sorted.
fn php_files(folder: &Path, below: &Path, depth: usize, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(below) else { return };
    let mut entries: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    entries.sort();
    for path in entries {
        if out.len() >= 2000 {
            return;
        }
        if path.is_dir() {
            if depth < 8 {
                php_files(folder, &path, depth + 1, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "php")
            && let Ok(relative) = path.strip_prefix(folder)
        {
            out.push(relative.with_extension("").to_string_lossy().replace('\\', "/"));
        }
    }
}

impl CodeIgniterName {
    fn project(cx: &RefContext<'_>) -> Arc<CodeIgniterProject> {
        cx.engine.codeigniter()
    }

    /// A filter's alias: without what follows a `:` (`auth:admin`). A
    /// table without its alias (`authors a`), a column without its table and
    /// what compares it (`books.price >`).
    fn key<'v>(&self, value: &'v str) -> &'v str {
        match self.source {
            Source::Filter => value.split(':').next().unwrap_or(value),
            Source::Table => value.split_whitespace().next().unwrap_or(value),
            Source::Column => {
                let word = value.split_whitespace().next().unwrap_or(value);
                word.rsplit('.').next().unwrap_or(word)
            }
            _ => value,
        }
    }

    /// CodeIgniter 3: the class a model or a library is named by, the last
    /// part of `admin/book_model` in any case.
    fn legacy_class(cx: &RefContext<'_>, value: &str, prefixes: &[&str]) -> Option<String> {
        let name = value.rsplit('/').next().unwrap_or(value);
        let resolver = cx.engine.resolver();
        prefixes
            .iter()
            .find_map(|prefix| resolver.class_model(&format!("{prefix}{name}")).map(|model| model.fqn().to_owned()))
    }

    /// CodeIgniter 3: where `books/show/3` goes, the method of the
    /// controller its segments name (folders first), else the controller.
    fn uri_targets(cx: &RefContext<'_>, value: &str) -> Vec<Target> {
        let project = Self::project(cx);
        if value.contains(['?', ':', '#', '.']) {
            return Vec::new();
        }
        let upper = |segment: &str| {
            let mut name = segment.to_owned();
            if let Some(first) = name.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            name
        };
        // CodeIgniter 4: a route of that path, before what auto-routing would find.
        if !project.legacy {
            let path = value.trim_matches('/');
            let routes: Vec<Target> = project
                .route_paths
                .iter()
                .filter(|route| route.name == path)
                .map(|route| Target { path: route.file.clone(), range: route.range })
                .collect();
            if !routes.is_empty() {
                return routes;
            }
        }
        let routed = project.routed(value);
        let value = routed.as_deref().unwrap_or(value);
        let mut folder = project.app().join(if project.legacy { "controllers" } else { "Controllers" });
        let mut namespace = "App\\Controllers".to_owned();
        let mut segments = value.trim_matches('/').split('/').filter(|segment| !segment.is_empty());
        while let Some(segment) = segments.next() {
            let below = [segment.to_owned(), upper(segment)].into_iter().find(|name| folder.join(name).is_dir());
            if let Some(below) = below {
                namespace.push_str(&format!("\\{}", upper(&below)));
                folder.push(below);
                continue;
            }
            let name = upper(segment);
            let file = [folder.join(format!("{name}.php")), folder.join(format!("{segment}.php"))]
                .into_iter()
                .find(|file| file.is_file());
            let Some(file) = file else { return Vec::new() };
            let method = segments.next().unwrap_or("index");
            let class = if project.legacy { name.clone() } else { format!("{namespace}\\{name}") };
            // (Auto-routing's methods start with the HTTP verb: `getShow`.)
            let mut methods = vec![method.to_owned()];
            if !project.legacy {
                methods.extend(
                    ["get", "post", "put", "patch", "delete"].iter().map(|verb| format!("{verb}{}", upper(method))),
                );
            }
            let found: Vec<Target> = methods.iter().filter_map(|method| method_target(cx, &class, method)).collect();
            return if found.is_empty() { vec![Target { path: file, range: Range::default() }] } else { found };
        }
        Vec::new()
    }

    /// The site's call or property, when the question is about one string.
    fn place<'c>(cx: &'c RefContext<'_>) -> Option<&'c CallTarget> {
        cx.site.as_ref()?.call.as_ref().map(|(target, _)| target)
    }

    /// Whether the string is in a property of the validation configuration
    /// that isn't a group of rules.
    fn not_rules(cx: &RefContext<'_>) -> bool {
        matches!(Self::place(cx), Some(CallTarget::Property { class, name })
            if VALIDATION_CONFIG.contains(&class.as_str())
                && (name == "ruleSets" || name == "templates" || name.ends_with("_errors")))
    }

    /// The table a model keeps its rows in: the default of its `$table`.
    fn model_table(cx: &RefContext<'_>, class: &str) -> Option<String> {
        let resolver = cx.engine.resolver();
        let model = resolver.class_model(class)?;
        if !MODELS.iter().any(|base| model.is_subclass_of(base)) {
            return None;
        }
        let default = model.properties.get("table")?.declared()?.default.clone()?;
        let table = default.trim().trim_matches(['\'', '"']).to_owned();
        (!table.is_empty() && table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(table)
    }

    /// The table of the column written `value`: the one written before it
    /// (`books.title`), the one the builder was made for, the model's.
    fn table(cx: &RefContext<'_>, value: &str) -> Option<String> {
        let word = value.split_whitespace().next().unwrap_or(value);
        if let Some((table, _)) = word.rsplit_once('.') {
            let names = cx.engine.names();
            if !names.named(&[DeclaredKind::CodeIgniterTable], table).is_empty() {
                return Some(table.to_owned());
            }
        }
        match Self::place(cx)? {
            CallTarget::Method { classes, tables, name, .. } => {
                if name.eq_ignore_ascii_case("dropColumn") || name.eq_ignore_ascii_case("drop_column") {
                    return cx.site.as_ref()?.first_argument.clone();
                }
                // CodeIgniter 3's builder doesn't say its table until the query is run:
                // `insert('books', [...])`, else the tables of the function around.
                if classes.iter().any(|class| LEGACY_DB.contains(&class.as_str())) {
                    if let Some(table) = cx.site.as_ref()?.first_argument.clone() {
                        return Some(table);
                    }
                    let found = Self::function_tables(cx);
                    let column = word.rsplit('.').next().unwrap_or(word);
                    let names = cx.engine.names();
                    let has = |table: &String| {
                        !names.named(&[DeclaredKind::CodeIgniterColumn], &format!("{table}.{column}")).is_empty()
                    };
                    return found.iter().find(|table| has(table)).or(found.first()).cloned();
                }
                tables.first().cloned().or_else(|| classes.iter().find_map(|class| Self::model_table(cx, class)))
            }
            CallTarget::Property { class, .. } => Self::model_table(cx, class),
            _ => None,
        }
    }

    /// CodeIgniter 3: the tables the function around the site names
    /// (`->get('books')`, `->from('books')`, `->join('authors', …)`).
    fn function_tables(cx: &RefContext<'_>) -> Vec<String> {
        let Some(site) = cx.site.as_ref() else { return Vec::new() };
        let text = match cx.engine.open_document(&site.path) {
            Some(document) => document.text.to_string(),
            None => std::fs::read_to_string(&site.path).unwrap_or_default(),
        };
        let at = (site.start as usize).min(text.len());
        let start = text[..at].rfind("function ").unwrap_or(0);
        let end = text[at..].find("function ").map_or(text.len(), |found| at + found);
        let body = &text[start..end];
        let mut out: Vec<String> = Vec::new();
        for method in LEGACY_TABLES {
            let call = format!("->{method}(");
            let mut from = 0;
            while let Some(found) = body[from..].find(&call) {
                from += found + call.len();
                let rest = body[from..].trim_start();
                let Some(quote) = rest.chars().next().filter(|c| matches!(c, '\'' | '"')) else { continue };
                let Some(close) = rest[1..].find(quote) else { continue };
                let table = rest[1..1 + close].split_whitespace().next().unwrap_or_default().to_owned();
                if !table.is_empty() && !out.contains(&table) {
                    out.push(table);
                }
            }
        }
        out
    }

    /// The table and the field of a rule that names them, `is_unique[books.title]`,
    /// with where each is in the string; `None` for another rule.
    fn rule_field<'v>(value: &'v str, segment: &RuleSegment<'v>) -> Option<(Span, Option<Span>)> {
        if !matches!(segment.name.trim(), "is_unique" | "is_not_unique") {
            return None;
        }
        let start = segment.name_end + 1;
        let end =
            value[start.min(segment.end)..segment.end].find([',', ']']).map_or(segment.end, |found| start + found);
        let first = value.get(start..end)?;
        Some(match first.find('.') {
            Some(dot) => ((start, start + dot), Some((start + dot + 1, end))),
            None => ((start, end), None),
        })
    }

    /// The column `value` names, when it is one name: not `COUNT(id)`, not
    /// `title, author`.
    fn column<'v>(&self, value: &'v str) -> Option<&'v str> {
        let key = self.key(value);
        (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then_some(key)
    }

    /// Where the column at byte `at` of `title, price` is: one of several
    /// in a string, without the spaces around it.
    fn column_segment(value: &str, at: usize) -> (usize, usize) {
        let at = at.min(value.len());
        let start = value[..at].rfind(',').map_or(0, |comma| comma + 1);
        let end = value[at..].find(',').map_or(value.len(), |comma| at + comma);
        let text = &value[start..end];
        let lead = text.len() - text.trim_start().len();
        (start + lead, start + text.trim_end().len().max(lead))
    }

    /// Whether a class of that short name is somewhere: CodeIgniter looks
    /// for a model, a configuration and a cell in every namespace.
    fn somewhere(cx: &RefContext<'_>, name: &str) -> bool {
        let short = name.rsplit('\\').next().unwrap_or(name);
        let Some(index) = cx.engine.index() else { return true };
        index
            .by_short_name(short)
            .map_or(true, |found| found.iter().any(|decl| matches!(decl.kind, crate::index::DeclKind::ClassLike(_))))
    }

    /// The namespace the controllers of the route at the site are in: the
    /// `namespace` of the group around it, else the default one.
    fn controllers(cx: &RefContext<'_>) -> String {
        let found = cx.site.as_ref().and_then(|site| {
            let text = match cx.engine.open_document(&site.path) {
                Some(document) => document.text.to_string(),
                None => std::fs::read_to_string(&site.path).ok()?,
            };
            let parsed = crate::syntax::ParsedFile::parse(&site.path.to_string_lossy(), text);
            crate::frameworks::codeigniter::project::route_namespace(&parsed, site.start)
        });
        found.unwrap_or_else(|| "App\\Controllers".to_owned())
    }

    /// The class and the method `Books::show/$1` names, of a class of
    /// `namespace` unless it is written from the root.
    fn handler(namespace: &str, value: &str) -> (String, String) {
        let handler = value.split('/').next().unwrap_or(value);
        let (class, method) = handler.split_once("::").unwrap_or((handler, "index"));
        let class = match class.strip_prefix('\\') {
            Some(fqn) => fqn.to_owned(),
            None => format!("{}\\{class}", namespace.trim_matches('\\')),
        };
        (class, method.to_owned())
    }

    /// Where the method a handler names is: the method, else the class.
    fn handler_targets(cx: &RefContext<'_>, namespace: &str, value: &str) -> Vec<Target> {
        let (class, method) = Self::handler(namespace, value);
        match method_target(cx, &class, &method) {
            Some(target) => vec![target],
            None => cx.engine.class_targets(&class),
        }
    }

    /// The classes whose methods are rules, when every one is known.
    fn rule_sets(cx: &RefContext<'_>) -> Option<Vec<String>> {
        let sets = Self::project(cx).rule_sets.clone();
        let resolver = cx.engine.resolver();
        sets.iter().all(|class| resolver.class_model(class).is_some()).then_some(sets)
    }

    /// The names declared of `kind`, each with where.
    fn declared(cx: &RefContext<'_>, kind: DeclaredKind) -> Vec<Def> {
        let names = cx.engine.names();
        names
            .of(&[kind])
            .map(|declared| Def {
                name: declared.name.clone(),
                detail: None,
                file: declared.path.clone(),
                range: declared.range,
            })
            .collect()
    }

    /// The names of this kind, each with where it is declared; a name of a
    /// file or a class has no place of its own here.
    fn defs(&self, cx: &RefContext<'_>) -> Vec<Def> {
        let project = Self::project(cx);
        let named = |name: String, file: PathBuf| Def { name, detail: None, file, range: Range::default() };
        match self.source {
            Source::Lang => project.lines.clone(),
            Source::Filter => project.filters.clone(),
            // By its `as`, else by its path.
            Source::Route => {
                let mut out = Self::declared(cx, DeclaredKind::CodeIgniterRoute);
                out.extend(project.route_paths.iter().cloned());
                out
            }
            Source::Event => Self::declared(cx, DeclaredKind::CodeIgniterEvent),
            Source::Table => Self::declared(cx, DeclaredKind::CodeIgniterTable),
            // A table's columns are asked for with the table (`complete`).
            Source::Column => Vec::new(),
            Source::View => {
                let mut out = Vec::new();
                for (prefix, folder) in project.view_dirs() {
                    let mut found = Vec::new();
                    php_files(&folder, &folder, 0, &mut found);
                    for name in found {
                        let file = folder.join(format!("{name}.php"));
                        out.push(named(format!("{prefix}{name}"), file));
                    }
                }
                out
            }
            Source::Rule => {
                let resolver = cx.engine.resolver();
                let mut out: Vec<Def> = Vec::new();
                for class in &project.rule_sets {
                    let Some(model) = resolver.class_model(class) else { continue };
                    let set = class.rsplit('\\').next().unwrap_or(class);
                    for method in model.methods.iter() {
                        let public = method
                            .declared()
                            .is_some_and(|declared| declared.visibility == crate::summary::Visibility::Public);
                        let name = &method.name;
                        if public && !name.starts_with("__") && !out.iter().any(|known| known.name == *name) {
                            out.push(Def {
                                name: name.clone(),
                                detail: Some(set.to_owned()),
                                file: PathBuf::new(),
                                range: Range::default(),
                            });
                        }
                    }
                }
                out.extend(BUILT_IN_RULES.iter().map(|name| named((*name).to_owned(), PathBuf::new())));
                out.sort_by(|a, b| a.name.cmp(&b.name));
                out
            }
            Source::RuleGroup => {
                let resolver = cx.engine.resolver();
                let Some(model) = resolver.class_model(VALIDATION_CONFIG[0]) else { return Vec::new() };
                let mut out = Vec::new();
                for property in model.properties.iter() {
                    let Some(declared) = property.declared() else { continue };
                    let name = &declared.name;
                    let own = property.declaring.fqn.trim_start_matches('\\') == VALIDATION_CONFIG[0];
                    if !own || name == "ruleSets" || name == "templates" || name.ends_with("_errors") {
                        continue;
                    }
                    let Some(file) = cx.engine.class_path(&property.declaring, Path::new(""), &FileSummary::default())
                    else {
                        continue;
                    };
                    out.push(Def { name: name.clone(), detail: None, file, range: declared.name_loc.range });
                }
                out
            }
            Source::Helper => {
                let mut out: Vec<Def> = Vec::new();
                for folder in project.helper_dirs() {
                    let mut found = Vec::new();
                    php_files(&folder, &folder, 8, &mut found);
                    for file in found {
                        if let Some(name) = file.strip_suffix("_helper")
                            && !out.iter().any(|known| known.name == name)
                        {
                            out.push(named(name.to_owned(), folder.join(format!("{file}.php"))));
                        }
                    }
                }
                out
            }
            // The classes of the namespace: `Config\Shop` is `Shop`.
            Source::Config | Source::Model | Source::Cell if !(project.legacy && self.source == Source::Model) => {
                let namespace = match self.source {
                    Source::Config => "Config",
                    Source::Model => "App\\Models",
                    _ => "App\\Cells",
                };
                let Some(index) = cx.engine.index() else { return Vec::new() };
                let mut out: Vec<Def> = index
                    .namespace_children(namespace)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|decl| matches!(decl.kind, crate::index::DeclKind::ClassLike(_)))
                    .map(|decl| named(decl.short_name().to_owned(), decl.path()))
                    .collect();
                out.sort_by(|a, b| a.name.cmp(&b.name));
                out
            }
            // A controller's method is found from the string; nothing is listed.
            Source::Controller | Source::Uri => Vec::new(),
            Source::ConfigItem => project.config_items.clone(),
            // CodeIgniter 3: the files of the models folder, of the libraries folders.
            Source::Model | Source::Library if project.legacy => {
                let folders = match self.source {
                    Source::Model => vec![project.app().join("models")],
                    _ => project.library_dirs(),
                };
                let mut out: Vec<Def> = Vec::new();
                for folder in folders {
                    let mut found = Vec::new();
                    php_files(&folder, &folder, 0, &mut found);
                    for file in found {
                        let name = file.to_ascii_lowercase();
                        if !out.iter().any(|known| known.name == name) && name != "index" {
                            out.push(named(name, folder.join(format!("{file}.php"))));
                        }
                    }
                }
                out
            }
            Source::Library | Source::Config | Source::Model | Source::Cell => Vec::new(),
            Source::Service => {
                let resolver = cx.engine.resolver();
                let mut out: Vec<Def> = Vec::new();
                for class in SERVICES {
                    let Some(model) = resolver.class_model(class) else { continue };
                    for method in model.methods.iter().filter(|method| method.is_static) {
                        let name = method.name.clone();
                        // Not the plumbing of the base class.
                        let plumbing = name.starts_with("__") || matches!(name.as_str(), "get" | "set" | "override");
                        if !plumbing && !out.iter().any(|known| known.name == name) {
                            out.push(Def { name, detail: None, file: PathBuf::new(), range: Range::default() });
                        }
                    }
                }
                out.sort_by(|a, b| a.name.cmp(&b.name));
                out
            }
        }
    }

    fn targets(&self, cx: &RefContext<'_>, value: &str) -> Vec<Target> {
        let value = self.key(value);
        match self.source {
            Source::Uri => Self::uri_targets(cx, value),
            // CodeIgniter 3's `redirect('books/show')`.
            Source::Route if Self::project(cx).legacy => Self::uri_targets(cx, value),
            Source::Model | Source::Library if Self::project(cx).legacy => {
                let prefixes: &[&str] = if self.source == Source::Model { &[""] } else { &["", "CI_"] };
                match Self::legacy_class(cx, value, prefixes) {
                    Some(class) => cx.engine.class_targets(&class),
                    None => {
                        let found = self.defs(cx).into_iter().find(|def| def.name == value.to_ascii_lowercase());
                        found.into_iter().map(|def| Target { path: def.file, range: def.range }).collect()
                    }
                }
            }
            Source::Config => cx.engine.class_targets(&class_of("Config", value)),
            Source::Model => cx.engine.class_targets(&class_of("App\\Models", value)),
            // `Books::show/$1`: the method of the controller, before what it is passed.
            Source::Controller => Self::handler_targets(cx, &Self::controllers(cx), value),
            // `route_to('Books::show')`: the route of that handler.
            Source::Route if value.contains("::") => Self::handler_targets(cx, "App\\Controllers", value),
            // `Card::render`, `Card` (its `render`), a class written out.
            Source::Cell => {
                let (class, method) = value.split_once("::").unwrap_or((value, "render"));
                let class = class_of("App\\Cells", class);
                match method_target(cx, &class, method) {
                    Some(target) => vec![target],
                    None => cx.engine.class_targets(&class),
                }
            }
            Source::View => {
                let file = Self::project(cx).view_file(value).filter(|file| file.is_file());
                file.into_iter().map(|path| Target { path, range: Range::default() }).collect()
            }
            Source::Rule => {
                let project = Self::project(cx);
                project.rule_sets.iter().filter_map(|class| method_target(cx, class, value)).take(1).collect()
            }
            Source::Column => {
                let (Some(table), Some(column)) = (Self::table(cx, value), self.column(value)) else {
                    return Vec::new();
                };
                let names = cx.engine.names();
                names
                    .named(&[DeclaredKind::CodeIgniterColumn], &format!("{table}.{column}"))
                    .into_iter()
                    .map(|declared| Target { path: declared.path.clone(), range: declared.range })
                    .collect()
            }
            Source::Service => SERVICES.iter().filter_map(|class| method_target(cx, class, value)).take(1).collect(),
            _ => self
                .defs(cx)
                .into_iter()
                .filter(|def| def.name == value)
                .map(|def| Target { path: def.file, range: def.range })
                .collect(),
        }
    }
}

impl ReferenceKind for CodeIgniterName {
    fn id(&self) -> &'static str {
        self.id
    }

    fn patterns(&self) -> &'static [PlacePattern] {
        self.patterns
    }

    fn canonical(&self, cx: &RefContext<'_>, value: &str) -> String {
        match self.source {
            // The view's file names it one way: `Blog\Views\sub\list` is `Blog\Views\sub/list`.
            Source::View if value.contains('\\') => {
                let project = Self::project(cx);
                let named = project.view_file(value).and_then(|file| project.view_name(&file));
                named.unwrap_or_else(|| value.to_owned())
            }
            // With its table before it: `books.title`.
            Source::Column => match (Self::table(cx, value), self.column(value)) {
                (Some(table), Some(column)) => format!("{table}.{column}"),
                _ => value.to_owned(),
            },
            _ => self.key(value).to_owned(),
        }
    }

    fn refers_to(&self, value: &str, key: &str) -> bool {
        match self.source {
            Source::Column => value == key,
            Source::View if key.contains('\\') => value.replace('/', "\\") == key.replace('/', "\\"),
            _ => self.key(value) == key,
        }
    }

    fn usage_text<'v>(&self, value: &'v str) -> &'v str {
        match self.source {
            Source::Column => value.rsplit('.').next().unwrap_or(value),
            _ => value,
        }
    }

    fn usage_words(&self, value: &str) -> Vec<String> {
        crate::index::words(self.usage_text(value))
    }

    fn complete(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<RefItem> {
        if self.source == Source::Rule {
            if Self::not_rules(cx) {
                return Vec::new();
            }
            let at = at.min(value.len());
            let Some(segment) = rule_segments(value).into_iter().find(|s| s.start <= at && at <= s.end) else {
                return Vec::new();
            };
            if at > segment.name_end {
                // `is_unique[books.title]`: the tables, then the table's columns.
                let Some((table, field)) = Self::rule_field(value, &segment) else { return Vec::new() };
                let names = cx.engine.names();
                let mut seen = std::collections::HashSet::new();
                let item = |name: &str, range: (usize, usize), detail: &str| RefItem {
                    label: name.to_owned(),
                    insert: name.to_owned(),
                    detail: Some(detail.to_owned()),
                    from: range.0,
                    to: range.1,
                };
                return match field {
                    Some(field) if at >= field.0 => {
                        let prefix = format!("{}.", &value[table.0..table.1]);
                        names
                            .of(&[DeclaredKind::CodeIgniterColumn])
                            .filter_map(|declared| declared.name.strip_prefix(&prefix))
                            .filter(|name| seen.insert((*name).to_owned()))
                            .map(|name| item(name, field, "column"))
                            .collect()
                    }
                    _ => names
                        .of(&[DeclaredKind::CodeIgniterTable])
                        .filter(|declared| seen.insert(declared.name.clone()))
                        .map(|declared| item(&declared.name, table, "table"))
                        .collect(),
                };
            }
            return self
                .defs(cx)
                .into_iter()
                .map(|def| RefItem {
                    label: def.name.clone(),
                    insert: def.name,
                    detail: def.detail.or_else(|| Some(self.label.to_owned())),
                    from: segment.start,
                    to: segment.name_end,
                })
                .collect();
        }
        if self.source == Source::Column {
            // The column the caret is in (`select('title, pri|')`).
            let (start, end) = Self::column_segment(value, at);
            let segment = &value[start..end];
            let Some(table) = Self::table(cx, segment) else { return Vec::new() };
            let prefix = format!("{table}.");
            // What is written before the column stays: its table.
            let before = start + segment.rfind('.').map_or(0, |dot| dot + 1);
            let names = cx.engine.names();
            let mut seen = std::collections::HashSet::new();
            return names
                .of(&[DeclaredKind::CodeIgniterColumn])
                .filter_map(|declared| declared.name.strip_prefix(&prefix))
                .filter(|name| seen.insert((*name).to_owned()))
                .map(|name| RefItem {
                    label: name.to_owned(),
                    insert: name.to_owned(),
                    detail: Some(table.clone()),
                    from: before,
                    to: end,
                })
                .collect();
        }
        let mut seen = std::collections::HashSet::new();
        self.defs(cx)
            .into_iter()
            .filter(|def| seen.insert(def.name.clone()))
            .map(|def| RefItem {
                label: def.name.clone(),
                insert: def.name,
                detail: def.detail.or_else(|| Some(self.label.to_owned())),
                from: 0,
                to: value.len(),
            })
            .collect()
    }

    fn resolve(&self, cx: &RefContext<'_>, value: &str, at: usize) -> Vec<Target> {
        let mut out: Vec<Target> = Vec::new();
        // The rule the caret is in.
        let value = match self.source {
            Source::Rule if Self::not_rules(cx) => return out,
            Source::Rule => {
                let at = at.min(value.len());
                let Some(segment) = rule_segments(value).into_iter().find(|s| s.start <= at && at <= s.end) else {
                    return out;
                };
                // What `is_unique[books.title]` names: the table, its column.
                if at > segment.name_end {
                    let Some((table, field)) = Self::rule_field(value, &segment) else { return out };
                    let names = cx.engine.names();
                    let table_name = &value[table.0..table.1];
                    let found = match field {
                        Some(field) if at >= field.0 => names.named(
                            &[DeclaredKind::CodeIgniterColumn],
                            &format!("{table_name}.{}", &value[field.0..field.1]),
                        ),
                        _ => names.named(&[DeclaredKind::CodeIgniterTable], table_name),
                    };
                    return found
                        .into_iter()
                        .map(|declared| Target { path: declared.path.clone(), range: declared.range })
                        .collect();
                }
                segment.name
            }
            // The column the caret is in, of several; in an expression, the name the caret is on.
            Source::Column => {
                let (start, end) = Self::column_segment(value, at);
                let segment = &value[start..end];
                if self.column(segment).is_some() {
                    segment
                } else {
                    let at = at.clamp(start, end);
                    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.';
                    let from = value[start..at].rfind(|c| !word(c)).map_or(start, |found| start + found + 1);
                    let to = value[at..end].find(|c| !word(c)).map_or(end, |found| at + found);
                    &value[from..to]
                }
            }
            _ => value,
        };
        for target in self.targets(cx, value) {
            if !out.contains(&target) {
                out.push(target);
            }
        }
        out
    }

    fn rename_target(&self, cx: &RefContext<'_>, value: &str, _at: usize) -> Option<RenameTarget> {
        if !matches!(self.source, Source::Route | Source::Filter | Source::Event) {
            return None;
        }
        let key = self.key(value);
        // A route is renamed by its `as`: not one named by its path or by its handler.
        let defs = match self.source {
            Source::Route => Self::declared(cx, DeclaredKind::CodeIgniterRoute),
            _ => self.defs(cx),
        };
        let defs: Vec<Def> = defs.into_iter().filter(|def| def.name == key).collect();
        if defs.is_empty() || defs.iter().any(|def| cx.engine.is_library(&def.file)) {
            return None;
        }
        // A route's declaration is a string of the kind too; a filter's alias is a key of a property.
        let declarations = match self.source {
            Source::Filter => defs.into_iter().map(|def| (def.file, def.range)).collect(),
            _ => Vec::new(),
        };
        Some(RenameTarget { key: key.to_owned(), range: (0, key.len()), declarations })
    }

    fn valid_name(&self, name: &str) -> bool {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    }

    fn checks_backslashes(&self) -> bool {
        matches!(self.source, Source::View | Source::Model | Source::Config | Source::Cell)
    }

    fn is_known(&self, cx: &RefContext<'_>, value: &str) -> bool {
        match self.source {
            Source::Rule => self.undefined(cx, value).is_none(),
            Source::Column => true,
            _ => !self.targets(cx, value).is_empty() || self.defs(cx).is_empty(),
        }
    }

    fn undefined(&self, cx: &RefContext<'_>, value: &str) -> Option<Undefined> {
        let project = Self::project(cx);
        let key = self.key(value);
        let unknown = || Undefined::warning(format!("Unknown {} '{key}'", self.label));
        // The framework's own files name things by rules of their own (the toolbar's views).
        let own =
            cx.site.as_ref().is_some_and(|site| project.system().is_some_and(|system| site.path.starts_with(system)));
        if key.is_empty() || own {
            return None;
        }
        // What a package registers when the application starts isn't in a file read here.
        let registrars = || {
            let resolver = cx.engine.resolver();
            project
                .namespaces
                .iter()
                .any(|(namespace, _)| resolver.class_model(&format!("{namespace}\\Config\\Registrar")).is_some())
        };
        let missing = match self.source {
            // CodeIgniter 3's `redirect()` takes a path.
            Source::Route if project.legacy => false,
            Source::View => {
                let told = !value.contains('\\') || project.knows_namespace(value);
                // `$this->include()` outside a class is a view's only in a view.
                let template = matches!(Self::place(cx), Some(CallTarget::Method { classes, .. })
                    if classes.iter().all(|class| class == crate::frameworks::refs::TEMPLATE_THIS));
                let view = cx.site.as_ref().is_some_and(|site| project.view_name(&site.path).is_some());
                told && (view || !template) && project.views_dir().is_dir() && self.targets(cx, value).is_empty()
            }
            Source::Helper => project.system().is_some() && self.targets(cx, value).is_empty(),
            // A line of a file that has lines: another file may be a package's.
            Source::Lang => {
                let (file, _) = key.split_once('.')?;
                let prefix = format!("{file}.");
                let below = format!("{key}.");
                let known = project.lines.iter().any(|line| line.name.starts_with(&prefix));
                known && !project.lines.iter().any(|line| line.name == key || line.name.starts_with(&below))
            }
            // `route_to('Books::show')`: the method is there.
            Source::Route if key.contains("::") => {
                let (class, method) = Self::handler("App\\Controllers", key);
                let resolver = cx.engine.resolver();
                resolver.class_model(&class).is_some_and(|model| model.method(&method).is_none())
            }
            Source::Route => {
                let plain = key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '\\'));
                plain && !self.defs(cx).is_empty() && self.targets(cx, value).is_empty()
            }
            Source::Filter | Source::RuleGroup => {
                // Any array may have a `filter` key: a route's options are in a routes file.
                let entry = matches!(Self::place(cx), Some(CallTarget::Entry { .. }));
                let routes = cx.site.as_ref().is_some_and(|site| {
                    site.path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("Routes.php"))
                });
                let alias = key.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
                (routes || !entry)
                    && alias
                    && !self.defs(cx).is_empty()
                    && !registrars()
                    && self.targets(cx, value).is_empty()
            }
            Source::Controller => {
                let (class, method) = Self::handler(&Self::controllers(cx), value);
                let resolver = cx.engine.resolver();
                return match resolver.class_model(&class) {
                    None => Some(Undefined::warning(format!("Controller '{class}' not found"))),
                    Some(model) if model.method(&method).is_none() && model.method("_remap").is_none() => {
                        Some(Undefined::warning(format!("Controller '{class}' has no method '{method}'")))
                    }
                    Some(_) => None,
                };
            }
            Source::Rule => {
                if Self::not_rules(cx) || value.contains(['{', '$']) {
                    return None;
                }
                let sets = Self::rule_sets(cx)?;
                let resolver = cx.engine.resolver();
                let known = |name: &str| {
                    BUILT_IN_RULES.contains(&name)
                        || sets
                            .iter()
                            .any(|class| resolver.class_model(class).is_some_and(|model| model.method(name).is_some()))
                };
                let unknown: Vec<&str> = rule_segments(value)
                    .into_iter()
                    .map(|segment| segment.name.trim())
                    .filter(|name| !name.is_empty() && !known(name))
                    .collect();
                return match unknown.as_slice() {
                    [] => None,
                    [one] => Some(Undefined::warning(format!("Unknown validation rule '{one}'"))),
                    many => Some(Undefined::warning(format!(
                        "Unknown validation rules {}",
                        many.iter().map(|name| format!("'{name}'")).collect::<Vec<_>>().join(", ")
                    ))),
                };
            }
            // A class of that name nowhere; a cell's method that isn't there.
            Source::Model | Source::Config | Source::Cell if !project.legacy => {
                let (class, method) = match self.source {
                    Source::Cell => value.split_once("::").map_or((value, "render"), |(class, method)| (class, method)),
                    _ => (value, ""),
                };
                let namespace = match self.source {
                    Source::Model => "App\\Models",
                    Source::Config => "Config",
                    _ => "App\\Cells",
                };
                let resolver = cx.engine.resolver();
                return match resolver.class_model(&class_of(namespace, class)) {
                    Some(model) if !method.is_empty() && model.method(method).is_none() => {
                        Some(Undefined::warning(format!("View cell '{class}' has no method '{method}'")))
                    }
                    Some(_) => None,
                    None if class.contains('\\') || !Self::somewhere(cx, class) => {
                        Some(Undefined::warning(format!("Unknown {} '{class}'", self.label)))
                    }
                    None => None,
                };
            }
            // No method of the services of the application, of the framework or of a module.
            Source::Service => {
                let resolver = cx.engine.resolver();
                let mut classes: Vec<String> = SERVICES.iter().map(|class| (*class).to_owned()).collect();
                classes
                    .extend(project.namespaces.iter().map(|(namespace, _)| format!("{namespace}\\Config\\Services")));
                let known: Vec<_> = classes.iter().filter_map(|class| resolver.class_model(class)).collect();
                !known.is_empty() && !known.iter().any(|model| model.method(key).is_some())
            }
            // Off unless turned on: an event a package fires, a table made outside the
            // migrations.
            Source::Event => !self.defs(cx).is_empty() && self.targets(cx, value).is_empty(),
            Source::Table => {
                let plain = key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                plain && !self.defs(cx).is_empty() && self.targets(cx, value).is_empty()
            }
            Source::Column => {
                if value.contains(',') {
                    return None;
                }
                let (table, column) = (Self::table(cx, value)?, self.column(value)?);
                let made = !cx.engine.names().named(&[DeclaredKind::CodeIgniterTable], &table).is_empty();
                return (made && self.targets(cx, value).is_empty())
                    .then(|| Undefined::warning(format!("Unknown column '{column}' of table '{table}'")));
            }
            _ => false,
        };
        missing.then(unknown)
    }

    fn hover(&self, cx: &RefContext<'_>, value: &str) -> Option<String> {
        if self.source == Source::Rule && (Self::not_rules(cx) || rule_segments(value).len() != 1) {
            return None;
        }
        let value = match self.source {
            Source::Rule => rule_segments(value).first().map_or(value, |segment| segment.name),
            _ => value,
        };
        let key = self.key(value);
        let target = self.targets(cx, value).into_iter().next()?;
        let root = cx.engine.root();
        let file = target.path.strip_prefix(&root).unwrap_or(&target.path).to_string_lossy().replace('\\', "/");
        let place =
            if target.range == Range::default() { file } else { format!("{file}:{}", target.range.start.line + 1) };
        let detail = self.defs(cx).into_iter().find(|def| def.name == key).and_then(|def| def.detail);
        let mut out = format!("**{}** `{key}`", self.label);
        if let Some(detail) = detail {
            out.push_str(&format!("\n\n`{detail}`"));
        }
        out.push_str(&format!("\n\n{place}"));
        Some(out)
    }
}
