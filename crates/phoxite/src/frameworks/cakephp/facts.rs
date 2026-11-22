//! What the code of a CakePHP class says the class has ([`ClassFacts`]),
//! read from its file without running it:
//!
//! - a controller's components (`$this->loadComponent('Flash')`), its
//!   default table (`$defaultTable`, `$modelClass`) and the tables it loads
//!   as properties (`$this->loadModel('Tags')`);
//! - a view's and a helper's helpers (`loadHelper()`, `addHelper()`,
//!   `$helpers`), a component's components (`$components`);
//! - a table's associations (`belongsTo('Authors')`, `hasMany()`,
//!   `hasOne()`, `belongsToMany()`), its behaviours (`addBehavior()`), its
//!   entity class (`setEntityClass()`) and its database table (`setTable()`).
//!
//! A name's options are read where they change what it is: `className`
//! (another class under the alias) and `propertyName` (an association's
//! property of the entity).

use std::path::{Path, PathBuf};

use mago_span::HasSpan;
use mago_syntax::cst::{
    Access, Argument, ArrayElement, Class, ClassLikeConstantSelector, ClassLikeMemberSelector, Expression, MethodCall,
    PropertyConcreteItem, Variable,
};
use mago_syntax::walker::Walker;

use crate::base::Range;
use crate::frameworks::php_array::{elements, string_literal};
use crate::names::{FileNames, NameKind, Resolution};
use crate::syntax::ParsedFile;

/// Something a class loads under a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loaded {
    /// The name as written: `Flash`, `Blog.Tags`.
    pub name: String,
    /// What the `className` option names instead.
    pub class_name: Option<String>,
    /// An association's `propertyName`.
    pub property: Option<String>,
    pub file: PathBuf,
    /// Where the name is written.
    pub range: Range,
}

impl Loaded {
    /// The name the class has it under: `Tags` of `Blog.Tags`.
    pub fn alias(&self) -> &str {
        super::project::plugin_split(&self.name).1
    }

    /// What names its class: the `className` option, else the name.
    pub fn target(&self) -> &str {
        self.class_name.as_deref().unwrap_or(&self.name)
    }
}

/// How a table is associated with another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssociationKind {
    BelongsTo,
    HasOne,
    HasMany,
    BelongsToMany,
}

impl AssociationKind {
    /// The association's class.
    pub fn class(self) -> &'static str {
        match self {
            Self::BelongsTo => "Cake\\ORM\\Association\\BelongsTo",
            Self::HasOne => "Cake\\ORM\\Association\\HasOne",
            Self::HasMany => "Cake\\ORM\\Association\\HasMany",
            Self::BelongsToMany => "Cake\\ORM\\Association\\BelongsToMany",
        }
    }

    /// Whether an entity has several of the other table's.
    pub fn many(self) -> bool {
        matches!(self, Self::HasMany | Self::BelongsToMany)
    }

    fn of(method: &str) -> Option<Self> {
        match method.to_ascii_lowercase().as_str() {
            "belongsto" => Some(Self::BelongsTo),
            "hasone" => Some(Self::HasOne),
            "hasmany" => Some(Self::HasMany),
            "belongstomany" => Some(Self::BelongsToMany),
            _ => None,
        }
    }
}

/// An association of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Association {
    pub kind: AssociationKind,
    pub loaded: Loaded,
}

impl Association {
    /// The property of the table's entity holding what is associated:
    /// `author` for `belongsTo('Authors')`, `comments` for
    /// `hasMany('Comments')`, or the `propertyName` option.
    pub fn property(&self) -> String {
        if let Some(property) = &self.loaded.property {
            return property.clone();
        }
        let name = super::inflect::underscore(self.loaded.alias());
        if self.kind.many() { name } else { super::inflect::singularize(&name) }
    }
}

/// See the module documentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClassFacts {
    pub components: Vec<Loaded>,
    pub helpers: Vec<Loaded>,
    pub behaviors: Vec<Loaded>,
    pub associations: Vec<Association>,
    /// The tables a controller loads as properties (`loadModel()`).
    pub tables: Vec<Loaded>,
    /// `setEntityClass()`: a class, or a name (`Article`, `Blog.Post`).
    pub entity_class: Option<String>,
    /// `setTable()`: the database table.
    pub table: Option<String>,
    /// `$defaultTable` or `$modelClass` of a controller.
    pub default_table: Option<String>,
    /// `setTheme()`, `setTemplatePath()` and `setLayoutPath()` with a string.
    pub theme: Option<String>,
    pub template_path: Option<String>,
    pub layout_path: Option<String>,
}

struct Reader<'a> {
    parsed: &'a ParsedFile,
    names: &'a FileNames,
    file: &'a Path,
    /// The class asked about, and whether the walker is in it.
    wanted: &'a str,
    inside: bool,
    out: ClassFacts,
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl Reader<'_> {
    fn range(&self, start: u32, end: u32) -> Range {
        self.parsed.line_index().range(start, end)
    }

    /// A string, or the class of `Foo::class`.
    fn named(&self, expr: &Expression<'_>) -> Option<String> {
        if let Some((text, _, _)) = string_literal(expr) {
            return Some(text);
        }
        let Expression::Access(Access::ClassConstant(access)) = expr else { return None };
        let ClassLikeConstantSelector::Identifier(constant) = &access.constant else { return None };
        let Expression::Identifier(identifier) = access.class else { return None };
        if !constant.value.eq_ignore_ascii_case(b"class") {
            return None;
        }
        let name = lossy(identifier.value());
        match self.names.scope_at(identifier.span().start.offset).resolve(NameKind::ClassLike, &name).resolution {
            Resolution::Fqn(fqn) | Resolution::Fallback { namespaced: fqn, .. } => {
                Some(fqn.trim_start_matches('\\').to_owned())
            }
            Resolution::Relative(_) => None,
        }
    }

    /// What is loaded under the name `name` with the options `options`.
    fn loaded(&self, name: (String, u32, u32), options: Option<&Expression<'_>>) -> Loaded {
        let (name, start, end) = name;
        let mut loaded = Loaded {
            name,
            class_name: None,
            property: None,
            file: self.file.to_path_buf(),
            range: self.range(start, end),
        };
        for element in options.map(|options| elements(options)).unwrap_or_default() {
            let ArrayElement::KeyValue(element) = element else { continue };
            match string_literal(element.key).map(|(key, _, _)| key).as_deref() {
                Some("className") => loaded.class_name = self.named(element.value),
                Some("propertyName") => loaded.property = self.named(element.value),
                _ => {}
            }
        }
        loaded
    }

    /// The names of `$components = ['Flash', 'Auth' => ['className' => …]]`.
    fn listed(&self, array: &Expression<'_>) -> Vec<Loaded> {
        let mut out = Vec::new();
        for element in elements(array) {
            match element {
                ArrayElement::KeyValue(element) => {
                    if let Some(name) = string_literal(element.key) {
                        out.push(self.loaded(name, Some(element.value)));
                    }
                }
                ArrayElement::Value(element) => {
                    if let Some(name) = string_literal(element.value) {
                        out.push(self.loaded(name, None));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

struct FactWalker;

impl<'a> Walker<'a, 'a, Reader<'a>> for FactWalker {
    fn walk_in_class(&self, class: &'a Class<'a>, cx: &mut Reader<'a>) {
        cx.inside = lossy(class.name.value).eq_ignore_ascii_case(cx.wanted);
    }

    fn walk_out_class(&self, _: &'a Class<'a>, cx: &mut Reader<'a>) {
        cx.inside = false;
    }

    fn walk_in_property_concrete_item(&self, item: &'a PropertyConcreteItem<'a>, cx: &mut Reader<'a>) {
        if !cx.inside {
            return;
        }
        match item.variable.name {
            b"$components" => cx.out.components.extend(cx.listed(item.value)),
            b"$helpers" => cx.out.helpers.extend(cx.listed(item.value)),
            b"$defaultTable" | b"$modelClass" => {
                if let Some(name) = cx.named(item.value).filter(|name| !name.is_empty()) {
                    cx.out.default_table = Some(name);
                }
            }
            _ => {}
        }
    }

    fn walk_in_method_call(&self, call: &'a MethodCall<'a>, cx: &mut Reader<'a>) {
        if !cx.inside {
            return;
        }
        let ClassLikeMemberSelector::Identifier(method) = &call.method else { return };
        let method = lossy(method.value);
        let arguments: Vec<&Expression<'_>> = call
            .argument_list
            .arguments
            .iter()
            .filter_map(|argument| match argument {
                Argument::Positional(argument) => Some(argument.value),
                Argument::Named(_) => None,
            })
            .collect();
        let Some(first) = arguments.first() else { return };
        // Where the class's templates are: on the view builder, or on a view.
        let text = string_literal(first).map(|(text, _, _)| text).filter(|text| !text.is_empty());
        match method.to_ascii_lowercase().as_str() {
            "settheme" => cx.out.theme = text.clone().or(cx.out.theme.take()),
            "settemplatepath" => cx.out.template_path = text.clone().or(cx.out.template_path.take()),
            "setlayoutpath" => cx.out.layout_path = text.clone().or(cx.out.layout_path.take()),
            _ => {}
        }
        let options = arguments.get(1).copied();
        let on_this = matches!(call.object, Expression::Variable(Variable::Direct(this)) if this.name == b"$this");
        // `$this->viewBuilder()->addHelper('Form')` in a controller.
        let on_builder = matches!(call.object, Expression::Call(mago_syntax::cst::Call::Method(inner))
            if matches!(&inner.method, ClassLikeMemberSelector::Identifier(name) if name.value.eq_ignore_ascii_case(b"viewBuilder")));
        if on_builder {
            match method.to_ascii_lowercase().as_str() {
                "addhelper" => {
                    if let Some(name) = string_literal(first) {
                        cx.out.helpers.push(cx.loaded(name, options));
                    }
                }
                "addhelpers" | "sethelpers" => cx.out.helpers.extend(cx.listed(first)),
                _ => {}
            }
            return;
        }
        if !on_this {
            return;
        }
        if let Some(kind) = AssociationKind::of(&method) {
            if let Some(name) = string_literal(first) {
                cx.out.associations.push(Association { kind, loaded: cx.loaded(name, options) });
            }
            return;
        }
        match method.to_ascii_lowercase().as_str() {
            "loadcomponent" => {
                if let Some(name) = string_literal(first) {
                    cx.out.components.push(cx.loaded(name, options));
                }
            }
            "loadhelper" | "addhelper" => {
                if let Some(name) = string_literal(first) {
                    cx.out.helpers.push(cx.loaded(name, options));
                }
            }
            "addhelpers" => cx.out.helpers.extend(cx.listed(first)),
            "addbehavior" => {
                if let Some(name) = string_literal(first) {
                    cx.out.behaviors.push(cx.loaded(name, options));
                }
            }
            "addbehaviors" => cx.out.behaviors.extend(cx.listed(first)),
            "loadmodel" => {
                if let Some(name) = string_literal(first) {
                    cx.out.tables.push(cx.loaded(name, options));
                }
            }
            "setentityclass" => cx.out.entity_class = cx.named(first).or(cx.out.entity_class.take()),
            "settable" => cx.out.table = cx.named(first).or(cx.out.table.take()),
            _ => {}
        }
    }
}

/// What the class named `short` of the file `path` (its text `text`) says
/// it has.
pub fn read(path: &Path, text: String, short: &str) -> ClassFacts {
    let parsed = ParsedFile::parse(&path.to_string_lossy(), text);
    let names = FileNames::resolve(&parsed);
    let mut reader =
        Reader { parsed: &parsed, names: &names, file: path, wanted: short, inside: false, out: ClassFacts::default() };
    FactWalker.walk_program(parsed.program(), &mut reader);
    reader.out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_class_says_what_it_loads() {
        let table = "<?php\nnamespace App\\Model\\Table;\n\nuse App\\Model\\Entity\\Story;\nuse Cake\\ORM\\Table;\n\nclass ArticlesTable extends Table\n{\n    public function initialize(array $config): void\n    {\n        $this->setTable('posts');\n        $this->setEntityClass(Story::class);\n        $this->addBehavior('Timestamp');\n        $this->belongsTo('Authors', ['className' => 'Users', 'propertyName' => 'writer']);\n        $this->hasMany('Comments');\n        $this->belongsToMany('Blog.Tags');\n        $this->hasOne('PostCategories');\n    }\n}\nclass Other extends Table\n{\n    public function initialize(array $config): void\n    {\n        $this->hasMany('Nothing');\n    }\n}\n";
        let facts = read(Path::new("ArticlesTable.php"), table.to_owned(), "ArticlesTable");
        assert_eq!(facts.table.as_deref(), Some("posts"));
        assert_eq!(facts.entity_class.as_deref(), Some("App\\Model\\Entity\\Story"));
        assert_eq!(facts.behaviors[0].name, "Timestamp");
        let associations: Vec<(AssociationKind, &str, &str, String)> = facts
            .associations
            .iter()
            .map(|found| (found.kind, found.loaded.alias(), found.loaded.target(), found.property()))
            .collect();
        assert_eq!(
            associations,
            [
                (AssociationKind::BelongsTo, "Authors", "Users", "writer".to_owned()),
                (AssociationKind::HasMany, "Comments", "Comments", "comments".to_owned()),
                (AssociationKind::BelongsToMany, "Tags", "Blog.Tags", "tags".to_owned()),
                (AssociationKind::HasOne, "PostCategories", "PostCategories", "post_category".to_owned()),
            ]
        );
        assert_eq!(facts.associations[0].loaded.range.start.line, 13);

        let controller = "<?php\nclass AppController extends Controller\n{\n    protected ?string $defaultTable = 'Articles';\n    public $components = ['Paginator', 'Auth' => ['className' => 'MyAuth']];\n    public function initialize(): void\n    {\n        $this->loadComponent('Flash');\n        $this->loadModel('Tags');\n        $this->viewBuilder()->addHelper('Markdown');\n    }\n}\n";
        let facts = read(Path::new("AppController.php"), controller.to_owned(), "AppController");
        let components: Vec<(&str, &str)> =
            facts.components.iter().map(|found| (found.alias(), found.target())).collect();
        assert_eq!(components, [("Paginator", "Paginator"), ("Auth", "MyAuth"), ("Flash", "Flash")]);
        assert_eq!(facts.default_table.as_deref(), Some("Articles"));
        assert_eq!(facts.tables[0].name, "Tags");
        assert_eq!(facts.helpers[0].name, "Markdown");
    }
}
