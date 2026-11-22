//! The classes Magento's code generator writes on demand, as declarations:
//! a project that never ran `setup:di:compile` (or keeps `generated/` out
//! of the editor) still has them.
//!
//! | Class | Is |
//! | --- | --- |
//! | `FooFactory` | `create(array $data = []): Foo`, for a class or an interface `Foo` |
//! | `Foo\Proxy`, `Foo\Interceptor` | a subclass of `Foo` (an implementation, for an interface) |
//! | `FooExtensionInterface` | a getter and a setter per attribute `extension_attributes.xml` declares for `FooInterface` |
//! | `FooExtension` | the class implementing it |
//!
//! Each is written as PHP and read like any file ([`FileSummary::extract`]),
//! so it is a class like the others: it has members, it is a type, and it
//! isn't reported as undefined. A class of that name the project really has
//! (in `generated/`) is found first and this isn't asked.

use super::project::MagentoProject;
use crate::summary::{ClassKind, ClassLike, FileSummary};
use crate::syntax::ParsedFile;

/// `gift_note` is `GiftNote`.
fn studly(code: &str) -> String {
    code.split(['_', '-'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// An attribute's type as PHPDoc writes it: a class with its leading
/// backslash, a scalar as it is, `[]` kept.
fn doc_type(ty: &str) -> String {
    let (base, list) = match ty.strip_suffix("[]") {
        Some(base) => (base, "[]"),
        None => (ty, ""),
    };
    let base = base.trim().trim_start_matches('\\');
    let scalar = ["int", "integer", "string", "bool", "boolean", "float", "double", "array", "mixed"];
    if scalar.contains(&base.to_ascii_lowercase().as_str()) {
        format!("{base}{list}")
    } else {
        format!("\\{base}{list}")
    }
}

/// The PHP of the generated class `fqn`, when it is one: `kind_of` says what
/// a class the project has is (`None` when it has no such class).
fn source(fqn: &str, project: &MagentoProject, kind_of: &dyn Fn(&str) -> Option<ClassKind>) -> Option<String> {
    let fqn = fqn.trim_start_matches('\\');
    let (namespace, short) = fqn.rsplit_once('\\')?;
    // `Foo\Proxy`, `Foo\Interceptor`.
    if matches!(short, "Proxy" | "Interceptor") {
        let relation = match kind_of(namespace)? {
            ClassKind::Interface => "implements",
            ClassKind::Class => "extends",
            _ => return None,
        };
        return Some(format!("<?php\nnamespace {namespace};\nclass {short} {relation} \\{namespace}\n{{\n}}\n"));
    }
    // `FooExtensionInterface` and `FooExtension`, of `FooInterface`.
    for (suffix, interface) in [("ExtensionInterface", true), ("Extension", false)] {
        let Some(stem) = short.strip_suffix(suffix).filter(|stem| !stem.is_empty()) else { continue };
        let of = format!("{namespace}\\{stem}Interface");
        let attributes: Vec<_> =
            project.extension_attributes.iter().filter(|attribute| attribute.of.eq_ignore_ascii_case(&of)).collect();
        if attributes.is_empty() && kind_of(&of).is_none() {
            continue;
        }
        let mut body = String::new();
        let mut seen: Vec<&str> = Vec::new();
        for attribute in attributes {
            if seen.contains(&attribute.code.as_str()) {
                continue;
            }
            seen.push(&attribute.code);
            let (name, ty) = (studly(&attribute.code), doc_type(&attribute.ty));
            let end = if interface { ";" } else { " { return $this; }" };
            let get = if interface { ";" } else { " { return null; }" };
            body.push_str(&format!("    /** @return {ty}|null */\n    public function get{name}(){get}\n"));
            body.push_str(&format!("    /**\n     * @param {ty} $value\n     * @return $this\n     */\n    public function set{name}($value){end}\n"));
        }
        let head = if interface {
            format!("interface {short} extends \\Magento\\Framework\\Api\\ExtensionAttributesInterface")
        } else {
            format!(
                "class {short} extends \\Magento\\Framework\\Api\\AbstractSimpleObject implements \\{namespace}\\{stem}ExtensionInterface"
            )
        };
        return Some(format!("<?php\nnamespace {namespace};\n{head}\n{{\n{body}}}\n"));
    }
    // `FooFactory`.
    let of = fqn.strip_suffix("Factory").filter(|of| !of.ends_with('\\'))?;
    kind_of(of)?;
    Some(format!(
        "<?php\nnamespace {namespace};\nclass {short}\n{{\n    /** @return \\{of} */\n    public function create(array $data = [])\n    {{\n    }}\n}}\n"
    ))
}

/// The declaration of the generated class `fqn`, or `None` when it isn't
/// one Magento would generate here.
pub fn class_like(
    fqn: &str,
    project: &MagentoProject,
    kind_of: &dyn Fn(&str) -> Option<ClassKind>,
) -> Option<ClassLike> {
    let text = source(fqn, project, kind_of)?;
    let parsed = ParsedFile::parse("generated.php", text);
    let summary = FileSummary::extract(&parsed);
    summary.class_like(fqn.trim_start_matches('\\')).cloned()
}

/// Whether `fqn` has the shape of a generated class's name: what is worth
/// asking [`class_like`] about.
pub fn is_candidate(fqn: &str) -> bool {
    let short = fqn.rsplit('\\').next().unwrap_or(fqn);
    fqn.contains('\\')
        && (matches!(short, "Proxy" | "Interceptor")
            || short.ends_with("Factory")
            || short.ends_with("Extension")
            || short.ends_with("ExtensionInterface"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frameworks::magento::project::ExtensionAttribute;

    #[test]
    fn generated_classes_are_written_from_what_the_project_has() {
        let mut project = MagentoProject::default();
        for (code, ty) in [("gift_note", "string"), ("related", "Shop\\Api\\Data\\ItemInterface[]")] {
            project.extension_attributes.push(ExtensionAttribute {
                of: "Shop\\Api\\Data\\ItemInterface".to_owned(),
                code: code.to_owned(),
                ty: ty.to_owned(),
                file: Default::default(),
                range: Default::default(),
            });
        }
        let kind_of = |class: &str| match class {
            "Shop\\Model\\Cart" => Some(ClassKind::Class),
            "Shop\\Api\\CartInterface" | "Shop\\Api\\Data\\ItemInterface" => Some(ClassKind::Interface),
            _ => None,
        };
        let methods = |fqn: &str| -> Option<Vec<String>> {
            let class = class_like(fqn, &project, &kind_of)?;
            Some(class.methods.iter().map(|method| method.name.clone()).collect())
        };
        assert_eq!(methods("Shop\\Model\\CartFactory"), Some(vec!["create".to_owned()]));
        assert_eq!(methods("Shop\\Api\\CartInterfaceFactory"), Some(vec!["create".to_owned()]));
        assert_eq!(methods("Shop\\Model\\NothingFactory"), None);
        let proxy = class_like("Shop\\Model\\Cart\\Proxy", &project, &kind_of).unwrap();
        assert_eq!(proxy.extends.first().map(String::as_str), Some("Shop\\Model\\Cart"));
        let proxy = class_like("Shop\\Api\\CartInterface\\Proxy", &project, &kind_of).unwrap();
        assert_eq!(proxy.implements.first().map(String::as_str), Some("Shop\\Api\\CartInterface"));
        assert!(class_like("Shop\\Model\\Nothing\\Interceptor", &project, &kind_of).is_none());
        assert_eq!(
            methods("Shop\\Api\\Data\\ItemExtensionInterface"),
            Some(vec![
                "getGiftNote".to_owned(),
                "setGiftNote".to_owned(),
                "getRelated".to_owned(),
                "setRelated".to_owned()
            ])
        );
        assert_eq!(methods("Shop\\Api\\Data\\ItemExtension").map(|found| found.len()), Some(4));
        assert!(
            is_candidate("Shop\\Model\\CartFactory") && !is_candidate("Shop\\Model\\Cart") && !is_candidate("Factory")
        );
    }
}
