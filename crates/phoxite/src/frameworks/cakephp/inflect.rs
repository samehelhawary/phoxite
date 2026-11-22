//! The words CakePHP's conventions turn into each other: a table class
//! `ArticlesTable` has the entity `Article`, the action `viewAll` the
//! template `view_all.php`, the association `Authors` the property `author`.
//!
//! The plural and singular forms follow English spelling rules and a list of
//! the irregular nouns schemas use, written for this file. A word the rules
//! get wrong is only a class or a file that isn't found: every guess made
//! with them is checked against what the project has.

/// Irregular nouns: singular and plural, lower case.
const IRREGULAR: &[(&str, &str)] = &[
    ("person", "people"),
    ("man", "men"),
    ("woman", "women"),
    ("child", "children"),
    ("foot", "feet"),
    ("tooth", "teeth"),
    ("goose", "geese"),
    ("mouse", "mice"),
    ("ox", "oxen"),
    ("datum", "data"),
    ("medium", "media"),
    ("criterion", "criteria"),
    ("phenomenon", "phenomena"),
    ("index", "indices"),
    ("matrix", "matrices"),
    ("vertex", "vertices"),
    ("axis", "axes"),
    ("analysis", "analyses"),
    ("basis", "bases"),
    ("crisis", "crises"),
    ("diagnosis", "diagnoses"),
    ("thesis", "theses"),
    ("quiz", "quizzes"),
    ("leaf", "leaves"),
    ("life", "lives"),
    ("knife", "knives"),
    ("wife", "wives"),
    ("half", "halves"),
    ("shelf", "shelves"),
    ("wolf", "wolves"),
    ("hero", "heroes"),
    ("potato", "potatoes"),
    ("tomato", "tomatoes"),
    ("echo", "echoes"),
    ("cache", "caches"),
    ("move", "moves"),
    ("curve", "curves"),
    ("wave", "waves"),
    ("status", "statuses"),
    ("alias", "aliases"),
    ("bus", "buses"),
    ("campus", "campuses"),
    ("virus", "viruses"),
    ("address", "addresses"),
];

/// Nouns with one form for both.
const UNCOUNTABLE: &[&str] = &[
    "equipment",
    "information",
    "money",
    "news",
    "series",
    "species",
    "sheep",
    "fish",
    "deer",
    "feedback",
    "metadata",
    "stock",
    "software",
    "hardware",
    "furniture",
    "research",
    "advice",
    "staff",
];

/// Where the last word of a CamelCase or snake_case name starts.
fn last_word(name: &str) -> usize {
    let underscore = name.rfind('_').map_or(0, |at| at + 1);
    let upper = name.char_indices().rev().find(|(_, c)| c.is_ascii_uppercase()).map_or(0, |(at, _)| at);
    underscore.max(upper)
}

/// `word` with the capital its original starts with.
fn cased(original: &str, word: &str) -> String {
    if original.starts_with(|c: char| c.is_ascii_uppercase()) {
        let mut chars = word.chars();
        match chars.next() {
            Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
            None => String::new(),
        }
    } else {
        word.to_owned()
    }
}

fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

fn plural_word(word: &str) -> String {
    if UNCOUNTABLE.contains(&word) {
        return word.to_owned();
    }
    if let Some((_, plural)) = IRREGULAR.iter().find(|(singular, _)| *singular == word) {
        return (*plural).to_owned();
    }
    if IRREGULAR.iter().any(|(_, plural)| *plural == word) {
        return word.to_owned();
    }
    let before = |suffix: &str| word.strip_suffix(suffix).and_then(|stem| stem.chars().last());
    if let Some(stem) = word.strip_suffix('y')
        && before("y").is_some_and(|c| !is_vowel(c))
    {
        return format!("{stem}ies");
    }
    if ["s", "x", "z", "ch", "sh"].iter().any(|end| word.ends_with(end)) {
        return format!("{word}es");
    }
    format!("{word}s")
}

fn singular_word(word: &str) -> String {
    if UNCOUNTABLE.contains(&word) {
        return word.to_owned();
    }
    if let Some((singular, _)) = IRREGULAR.iter().find(|(_, plural)| *plural == word) {
        return (*singular).to_owned();
    }
    if IRREGULAR.iter().any(|(singular, _)| *singular == word) {
        return word.to_owned();
    }
    if let Some(stem) = word.strip_suffix("ies")
        && stem.len() > 1
    {
        return format!("{stem}y");
    }
    for end in ["sses", "xes", "zes", "ches", "shes"] {
        if word.ends_with(end) {
            return word[..word.len() - 2].to_owned();
        }
    }
    if word.ends_with("ss") || word.ends_with("us") || word.ends_with("is") {
        return word.to_owned();
    }
    word.strip_suffix('s').unwrap_or(word).to_owned()
}

/// `Article` is `Articles`, `post_category` is `post_categories`: the last
/// word made plural.
pub fn pluralize(name: &str) -> String {
    let at = last_word(name);
    let word = &name[at..];
    format!("{}{}", &name[..at], cased(word, &plural_word(&word.to_ascii_lowercase())))
}

/// `Articles` is `Article`, `PostCategories` is `PostCategory`: the last
/// word made singular.
pub fn singularize(name: &str) -> String {
    let at = last_word(name);
    let word = &name[at..];
    format!("{}{}", &name[..at], cased(word, &singular_word(&word.to_ascii_lowercase())))
}

/// `viewAll` and `ViewAll` are `view_all`.
pub fn underscore(name: &str) -> String {
    let mut out = String::new();
    for (at, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if at > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '-' || c == ' ' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

/// `view_all` and `view-all` are `ViewAll`.
pub fn camelize(name: &str) -> String {
    name.split(['_', '-', ' '])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// `view_all` and `ViewAll` are `viewAll`: a variable's or a method's name.
pub fn variable(name: &str) -> String {
    let camel = camelize(&underscore(name));
    let mut chars = camel.chars();
    match chars.next() {
        Some(first) => first.to_ascii_lowercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_turn_into_each_other() {
        for (singular, plural) in [
            ("Article", "Articles"),
            ("Category", "Categories"),
            ("PostCategory", "PostCategories"),
            ("Person", "People"),
            ("Address", "Addresses"),
            ("Box", "Boxes"),
            ("Status", "Statuses"),
            ("Day", "Days"),
            ("OrderItem", "OrderItems"),
            ("News", "News"),
            ("Bookmark", "Bookmarks"),
            ("Tag", "Tags"),
            ("Class", "Classes"),
        ] {
            assert_eq!(pluralize(singular), plural, "{singular}");
            assert_eq!(singularize(plural), singular, "{plural}");
        }
        assert_eq!(singularize("post_categories"), "post_category");
        assert_eq!(underscore("viewAll"), "view_all");
        assert_eq!(underscore("PostCategories"), "post_categories");
        assert_eq!(camelize("view_all"), "ViewAll");
        assert_eq!(camelize("my-action"), "MyAction");
        assert_eq!(variable("ViewAll"), "viewAll");
        assert_eq!(variable("view_all"), "viewAll");
    }
}
