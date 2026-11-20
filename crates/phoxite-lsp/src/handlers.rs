//! The requests. Each runs on its own thread with a snapshot of its
//! document; `Ok(None)` answers `null`.

use std::path::{Path, PathBuf};

use lsp_types::{self as lsp, Url};
use phoxite::base::{self as e, CancellationToken};
use phoxite::blade::views::{ViewPaths, ViewRef, goto_view, view_ref_at};
use phoxite::blade::{DIRECTIVES, compile, directive_prefix};
use phoxite::complete::{Completion, CompletionData, Completions};
use phoxite::engine::PhpEngine;
use phoxite::imports::ImportAction;
use phoxite::navigate::{Direction, HierarchyEntry, Target, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::convert::{self, location, lsp_pos, lsp_range, pos, range};
use crate::state::{DocSnapshot, Kind, State};

pub type Result<T> = std::result::Result<T, String>;

fn engine_error(e: e::IdeError) -> String {
    e.message
}

fn snapshot(state: &State, uri: &Url) -> Result<DocSnapshot> {
    state.snapshot(uri).ok_or_else(|| format!("{uri} is not open"))
}

fn locations(targets: impl IntoIterator<Item = (PathBuf, e::Range)>) -> Vec<lsp::Location> {
    targets.into_iter().filter_map(|(path, range)| location(&path, range)).collect()
}

fn targets(found: Vec<Target>) -> Vec<lsp::Location> {
    locations(found.into_iter().map(|t| (t.path, t.range)))
}

fn usages(found: Vec<Usage>) -> Vec<lsp::Location> {
    locations(found.into_iter().map(|u| (u.path, u.range)))
}

fn markup(state: &State, value: String) -> lsp::MarkupContent {
    let kind = if state.support.markdown { lsp::MarkupKind::Markdown } else { lsp::MarkupKind::PlainText };
    lsp::MarkupContent { kind, value }
}

// ---- completion ----

/// What a completion item carries for `completionItem/resolve`.
#[derive(Serialize, Deserialize)]
struct ItemData {
    uri: Url,
    php: CompletionData,
}

fn completion_item(state: &State, uri: &Url, item: Completion) -> lsp::CompletionItem {
    let new_text = item.insert_text;
    let text_edit = match item.replace_tab {
        Some(replace) if state.support.insert_replace => {
            lsp::CompletionTextEdit::InsertAndReplace(lsp::InsertReplaceEdit {
                new_text,
                insert: lsp_range(item.replace),
                replace: lsp_range(replace),
            })
        }
        _ => lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(item.replace), new_text)),
    };
    let snippet = item.is_snippet && state.support.snippets;
    lsp::CompletionItem {
        label: item.label,
        label_details: item
            .label_detail
            .map(|detail| lsp::CompletionItemLabelDetails { detail: Some(detail), description: None }),
        kind: Some(convert::completion_kind(item.kind)),
        detail: item.detail,
        tags: item.deprecated.then(|| vec![lsp::CompletionItemTag::DEPRECATED]),
        sort_text: Some(item.sort_text),
        filter_text: item.filter_text,
        insert_text_format: Some(if snippet {
            lsp::InsertTextFormat::SNIPPET
        } else {
            lsp::InsertTextFormat::PLAIN_TEXT
        }),
        text_edit: Some(text_edit),
        additional_text_edits: (!item.additional_edits.is_empty())
            .then(|| item.additional_edits.into_iter().map(convert::text_edit).collect()),
        data: item.data.and_then(|php| serde_json::to_value(ItemData { uri: uri.clone(), php }).ok()),
        ..lsp::CompletionItem::default()
    }
}

/// Directive names, when the engine isn't running.
fn directive_items(text: &str, at: e::Pos) -> Vec<lsp::CompletionItem> {
    let Some((_, replace)) = directive_prefix(text, at) else { return Vec::new() };
    DIRECTIVES
        .iter()
        .map(|name| lsp::CompletionItem {
            label: format!("@{name}"),
            kind: Some(lsp::CompletionItemKind::KEYWORD),
            text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(replace), format!("@{name}")))),
            ..lsp::CompletionItem::default()
        })
        .collect()
}

/// The ids of the Symfony reference kinds a template writes names of.
const SYMFONY_ROUTE: &str = "symfonyRoute";
const SYMFONY_TEMPLATE: &str = "symfonyTemplate";
const SYMFONY_TRANSLATION: &str = "symfonyTranslation";

/// The route or template name at `at` of a Twig template: the kind's id, and
/// the name with its range.
fn twig_name_at(text: &str, at: e::Pos) -> Option<(&'static str, phoxite::twig::TemplateRef)> {
    if let Some(route) = phoxite::twig::route_at(text, at) {
        return Some((SYMFONY_ROUTE, route));
    }
    if let Some(key) = phoxite::twig::translation_at(text, at) {
        return Some((SYMFONY_TRANSLATION, key));
    }
    phoxite::twig::template_at(text, at).map(|template| (SYMFONY_TEMPLATE, template))
}

/// The member at `at` of a variable the project's PHP passes to the
/// template (`post.ti|tle`): where it is declared, its type, its name and
/// the name's range.
fn twig_member(
    doc: &DocSnapshot,
    project: &phoxite::twig::TwigProject,
    at: e::Pos,
) -> Option<(Vec<phoxite::navigate::Target>, String, String, e::Range)> {
    let engine = doc.engine.as_ref()?;
    let template = project.paths.name_of(&doc.path)?;
    let (chain, range) = phoxite::twig::access_chain_at(&doc.text, at)?;
    let index = phoxite::syntax::LineIndex::new(&doc.text);
    let (start, end) = index.byte_range(range);
    let name = doc.text.get(start as usize..end as usize).filter(|name| !name.is_empty())?.to_owned();
    let loops: Vec<phoxite::frameworks::symfony::LoopVariable> = phoxite::twig::loops_at(&doc.text, at)
        .into_iter()
        .map(|(name, source)| phoxite::frameworks::symfony::LoopVariable { name, source })
        .collect();
    let (targets, ty) = engine.twig_member(&template, &loops, &chain, &name)?;
    Some((targets, ty, name, range))
}

/// The kind of Twig name a name added by PHP is.
fn twig_kind(kind: phoxite::summary::TwigNameKind) -> phoxite::twig::TwigCompletionKind {
    use phoxite::summary::TwigNameKind as Added;
    use phoxite::twig::TwigCompletionKind as Kind;
    match kind {
        Added::Filter => Kind::Filter,
        Added::Function => Kind::Function,
        Added::Test => Kind::Test,
    }
}

/// The names of kind `kind` a Twig template writes.
fn twig_names(text: &str, kind: &str) -> Vec<phoxite::twig::TemplateRef> {
    match kind {
        SYMFONY_ROUTE => phoxite::twig::route_refs(text),
        SYMFONY_TRANSLATION => phoxite::twig::translation_refs(text),
        _ => phoxite::twig::template_refs(text),
    }
}

fn twig_completion_item(state: &State, item: phoxite::twig::TwigCompletion) -> lsp::CompletionItem {
    use phoxite::twig::TwigCompletionKind as K;
    let kind = match item.kind {
        K::Variable => lsp::CompletionItemKind::VARIABLE,
        K::Macro | K::Function | K::Filter | K::Test => lsp::CompletionItemKind::FUNCTION,
        K::Tag => lsp::CompletionItemKind::KEYWORD,
        K::Template => lsp::CompletionItemKind::FILE,
    };
    lsp::CompletionItem {
        label: item.label,
        kind: Some(kind),
        detail: item.detail,
        documentation: item.documentation.map(|text| lsp::Documentation::MarkupContent(markup(state, text))),
        sort_text: Some(item.sort_text),
        text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(item.replace), item.insert_text))),
        ..lsp::CompletionItem::default()
    }
}

pub fn completion(
    state: &State,
    p: lsp::CompletionParams,
    cancel: &CancellationToken,
) -> Result<Option<lsp::CompletionResponse>> {
    let uri = p.text_document_position.text_document.uri;
    let doc = snapshot(state, &uri)?;
    let at = pos(p.text_document_position.position);
    let trigger = p.context.and_then(|c| c.trigger_character);
    if doc.kind == Kind::Twig {
        let project = state.twig_project(&doc.root);
        let mut items: Vec<lsp::CompletionItem> =
            phoxite::twig::completions(&doc.text, at, project.symfony, &project.paths)
                .into_iter()
                .map(|item| twig_completion_item(state, item))
                .collect();
        // What the project's PHP passes to the template, and what follows a `.` after it.
        if let Some(engine) = &doc.engine
            && let Some(template) = project.paths.name_of(&doc.path)
        {
            if let Some((chain, replace)) = phoxite::twig::access_chain_at(&doc.text, at) {
                let loops: Vec<phoxite::frameworks::symfony::LoopVariable> = phoxite::twig::loops_at(&doc.text, at)
                    .into_iter()
                    .map(|(name, source)| phoxite::frameworks::symfony::LoopVariable { name, source })
                    .collect();
                items.extend(engine.twig_members(&template, &loops, &chain).into_iter().map(|member| {
                    lsp::CompletionItem {
                        label: member.name.clone(),
                        kind: Some(if member.is_method {
                            lsp::CompletionItemKind::METHOD
                        } else {
                            lsp::CompletionItemKind::PROPERTY
                        }),
                        detail: Some(member.detail),
                        text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(
                            lsp_range(replace),
                            member.name,
                        ))),
                        ..lsp::CompletionItem::default()
                    }
                }));
            } else if let Some((phoxite::twig::TwigCompletionKind::Function, replace)) =
                phoxite::twig::name_kind_at(&doc.text, at)
            {
                for (name, ty) in engine.twig_variables(&template) {
                    if items.iter().any(|item| item.label == name) {
                        continue;
                    }
                    items.push(lsp::CompletionItem {
                        label: name.clone(),
                        kind: Some(lsp::CompletionItemKind::VARIABLE),
                        detail: Some(ty),
                        sort_text: Some(format!("0{name}")),
                        text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(replace), name))),
                        ..lsp::CompletionItem::default()
                    });
                }
            }
        }
        // The filters, functions and tests the project's own extensions add.
        if let Some((kind, replace)) = phoxite::twig::name_kind_at(&doc.text, at)
            && let Some(engine) = &doc.engine
        {
            for name in engine.twig_extension_names().into_iter().filter(|name| twig_kind(name.kind) == kind) {
                if items.iter().any(|item| item.label == name.name) {
                    continue;
                }
                items.push(lsp::CompletionItem {
                    label: name.name.clone(),
                    kind: Some(lsp::CompletionItemKind::FUNCTION),
                    detail: Some("the project's".to_owned()),
                    sort_text: Some(format!("2{}", name.name)),
                    text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(replace), name.name))),
                    ..lsp::CompletionItem::default()
                });
            }
        }
        // Before `|trans`: the project's translation keys.
        if let Some(replace) = phoxite::twig::translation_string_at(&doc.text, at)
            && let Some(engine) = &doc.engine
        {
            items.extend(engine.symfony_name_items(SYMFONY_TRANSLATION).into_iter().map(|key| lsp::CompletionItem {
                label: key.label,
                kind: Some(lsp::CompletionItemKind::VALUE),
                detail: key.detail,
                text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(replace), key.insert))),
                ..lsp::CompletionItem::default()
            }));
        }
        // In `path('…')` and `url('…')`: the project's route names.
        if let Some(replace) = phoxite::twig::route_string_at(&doc.text, at)
            && let Some(engine) = &doc.engine
        {
            items.extend(engine.symfony_name_items(SYMFONY_ROUTE).into_iter().map(|route| lsp::CompletionItem {
                label: route.label,
                kind: Some(lsp::CompletionItemKind::VALUE),
                detail: route.detail,
                text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(lsp_range(replace), route.insert))),
                ..lsp::CompletionItem::default()
            }));
        }
        return Ok(Some(lsp::CompletionResponse::List(lsp::CompletionList { is_incomplete: false, items })));
    }
    if doc.kind == Kind::WordPressScript {
        let found = doc.engine.as_ref().and_then(|engine| engine.wordpress_script_completion(&doc.path, &doc.text, at));
        return Ok(found.map(|found| {
            let items = found
                .into_iter()
                .map(|item| lsp::CompletionItem {
                    label: item.label.clone(),
                    kind: Some(lsp::CompletionItemKind::VALUE),
                    detail: Some(item.detail.to_owned()),
                    text_edit: Some(lsp::CompletionTextEdit::Edit(lsp::TextEdit::new(
                        lsp_range(item.range),
                        item.label,
                    ))),
                    ..lsp::CompletionItem::default()
                })
                .collect();
            lsp::CompletionResponse::List(lsp::CompletionList { is_incomplete: false, items })
        }));
    }
    // `|` opens completion in Twig only (its filters).
    if trigger.as_deref() == Some("|") {
        return Ok(None);
    }
    let found: Option<Completions> = match (&doc.engine, doc.kind) {
        (Some(engine), Kind::Php) => engine.completion(doc.id, at, trigger.as_deref(), cancel),
        (Some(engine), Kind::Blade) => engine
            .blade_directive_completion(&doc.text, at)
            .or_else(|| engine.blade_laravel_completion(&doc.blade(), at))
            .or_else(|| engine.blade_completion(&doc.blade(), at, trigger.as_deref(), cancel)),
        (None, Kind::Blade) => {
            let items = directive_items(&doc.text, at);
            return Ok(Some(lsp::CompletionResponse::List(lsp::CompletionList { is_incomplete: false, items })));
        }
        _ => None,
    };
    let Some(found) = found else { return Ok(None) };
    let items = found.items.into_iter().map(|item| completion_item(state, &uri, item)).collect();
    Ok(Some(lsp::CompletionResponse::List(lsp::CompletionList { is_incomplete: found.incomplete, items })))
}

pub fn resolve_completion(state: &State, mut item: lsp::CompletionItem) -> Result<lsp::CompletionItem> {
    if item.documentation.is_some() {
        return Ok(item);
    }
    let Some(data) = item.data.clone().and_then(|data| serde_json::from_value::<ItemData>(data).ok()) else {
        return Ok(item);
    };
    if let Some(doc) = state.snapshot(&data.uri)
        && let Some(engine) = &doc.engine
        && let Some(markdown) = engine.completion_documentation(doc.id, &data.php)
    {
        item.documentation = Some(lsp::Documentation::MarkupContent(markup(state, markdown)));
    }
    Ok(item)
}

// ---- hover and signature help ----

pub fn hover(state: &State, p: lsp::HoverParams) -> Result<Option<lsp::Hover>> {
    let doc = snapshot(state, &p.text_document_position_params.text_document.uri)?;
    let at = pos(p.text_document_position_params.position);
    if doc.kind == Kind::Twig {
        let project = state.twig_project(&doc.root);
        // A member of what the controller passes: its type.
        let member = || {
            let (targets, ty, name, range) = twig_member(&doc, &project, at)?;
            let _ = targets;
            Some((format!("```php\n{name}: {ty}\n```"), range))
        };
        let found = phoxite::twig::hover(&doc.text, at, project.symfony).or_else(member);
        return Ok(found.map(|(markdown, range)| lsp::Hover {
            contents: lsp::HoverContents::Markup(markup(state, markdown)),
            range: Some(lsp_range(range)),
        }));
    }
    let Some(engine) = &doc.engine else { return Ok(None) };
    let found = match doc.kind {
        Kind::Php => engine.hover(doc.id, at),
        Kind::Blade => engine.blade_hover(&doc.blade(), at),
        Kind::WordPressScript => engine.wordpress_script_hover(&doc.path, &doc.text, at),
        _ => None,
    };
    Ok(found.map(|hover| lsp::Hover {
        contents: lsp::HoverContents::Markup(markup(state, hover.markdown)),
        range: Some(lsp_range(hover.range)),
    }))
}

pub fn signature_help(state: &State, p: lsp::SignatureHelpParams) -> Result<Option<lsp::SignatureHelp>> {
    let doc = snapshot(state, &p.text_document_position_params.text_document.uri)?;
    let at = pos(p.text_document_position_params.position);
    let Some(engine) = &doc.engine else { return Ok(None) };
    let found = match doc.kind {
        Kind::Php => engine.signature_help(doc.id, at),
        Kind::Blade => engine.blade_signature_help(&doc.blade(), at),
        _ => None,
    };
    Ok(found.map(|help| lsp::SignatureHelp {
        signatures: help
            .signatures
            .into_iter()
            .map(|s| lsp::SignatureInformation {
                label: s.label,
                documentation: s.documentation.map(|value| lsp::Documentation::MarkupContent(markup(state, value))),
                parameters: Some(
                    s.parameters
                        .into_iter()
                        .map(|(start, end)| lsp::ParameterInformation {
                            label: lsp::ParameterLabel::LabelOffsets([start, end]),
                            documentation: None,
                        })
                        .collect(),
                ),
                active_parameter: None,
            })
            .collect(),
        active_signature: Some(0),
        active_parameter: Some(help.active_parameter),
    }))
}

// ---- navigation ----

#[derive(Clone, Copy)]
pub enum Goto {
    Definition,
    TypeDefinition,
    Implementation,
}

pub fn goto(
    state: &State,
    p: lsp::TextDocumentPositionParams,
    kind: Goto,
) -> Result<Option<lsp::GotoDefinitionResponse>> {
    let doc = snapshot(state, &p.text_document.uri)?;
    let at = pos(p.position);
    let found = match (doc.kind, kind) {
        (Kind::Php, _) => {
            let Some(engine) = &doc.engine else { return Ok(None) };
            match kind {
                Goto::Definition => engine.definition(doc.id, at).map(targets),
                Goto::TypeDefinition => engine.type_definition(doc.id, at).map(targets),
                Goto::Implementation => match engine.implementations(doc.id, &[at], &CancellationToken::new()) {
                    Some(Ok(mut lists)) => Some(targets(lists.pop().unwrap_or_default())),
                    Some(Err(e)) => return Err(engine_error(e)),
                    None => None,
                },
            }
        }
        (Kind::Blade, Goto::Definition) => blade_definition(&doc, at),
        (Kind::Twig, Goto::Definition) => {
            // A route name or a translation key: where the project declares it.
            if let Some((kind, named)) = twig_name_at(&doc.text, at).filter(|(kind, _)| *kind != SYMFONY_TEMPLATE) {
                let found = doc.engine.as_ref().and_then(|engine| engine.symfony_name_targets(kind, &named.name));
                return Ok(found.map(|found| lsp::GotoDefinitionResponse::Array(targets(found))));
            }
            let project = state.twig_project(&doc.root);
            let found = phoxite::twig::definition(&doc.text, at, &project.paths);
            // A macro of the template itself is in the document's own file.
            let mut found: Vec<(PathBuf, e::Range)> = found
                .into_iter()
                .map(|target| (target.path.unwrap_or_else(|| doc.path.clone()), target.range))
                .collect();
            // A member of what the controller passes: the property, or the method Twig calls.
            if found.is_empty()
                && let Some((declared, ..)) = twig_member(&doc, &project, at)
            {
                found.extend(declared.into_iter().map(|target| (target.path, target.range)));
            }
            // A filter, function or test of the project's own: where PHP names it.
            if found.is_empty()
                && let Some((kind, range)) = phoxite::twig::name_kind_at(&doc.text, at)
                && let Some(engine) = &doc.engine
            {
                let index = phoxite::syntax::LineIndex::new(&doc.text);
                let (start, end) = index.byte_range(range);
                let word = &doc.text[start as usize..end as usize];
                found.extend(
                    engine
                        .twig_extension_names()
                        .into_iter()
                        .filter(|name| name.name == word && twig_kind(name.kind) == kind)
                        .filter_map(|name| Some((name.file?, name.range))),
                );
            }
            Some(locations(found))
        }
        (Kind::WordPressScript, Goto::Definition) => doc
            .engine
            .as_ref()
            .and_then(|engine| engine.wordpress_script_definition(&doc.path, &doc.text, at))
            .map(targets),
        _ => None,
    };
    Ok(found.map(lsp::GotoDefinitionResponse::Array))
}

/// The views and components a template names, else the targets of its PHP.
fn blade_definition(doc: &DocSnapshot, at: e::Pos) -> Option<Vec<lsp::Location>> {
    let engine = doc.engine.as_ref();
    if let Some(found) = engine.and_then(|engine| engine.blade_laravel_definition(&doc.blade(), at))
        && !found.is_empty()
    {
        return Some(targets(found));
    }
    let offset = phoxite::syntax::LineIndex::new(&doc.text).byte(at);
    if let Some(found) = engine.and_then(|engine| engine.blade_vue_definition(&doc.blade(), offset)) {
        return Some(targets(found));
    }
    let files = goto_view(&doc.text, at, &ViewPaths::new(&doc.root));
    if !files.is_empty() {
        return Some(locations(files.into_iter().map(|path| (path, e::Range::default()))));
    }
    engine.and_then(|engine| engine.blade_definition(&doc.blade(), at)).map(targets)
}

pub fn references(
    state: &State,
    p: lsp::ReferenceParams,
    cancel: &CancellationToken,
) -> Result<Option<Vec<lsp::Location>>> {
    let doc = snapshot(state, &p.text_document_position.text_document.uri)?;
    let at = pos(p.text_document_position.position);
    let include = p.context.include_declaration;
    if doc.kind == Kind::Twig {
        // The usages of the route or the template named at the caret.
        let Some((kind, named)) = twig_name_at(&doc.text, at) else { return Ok(None) };
        let project = state.twig_project(&doc.root);
        // In a Symfony project: PHP's usages and the templates' (with the declaration when asked).
        let known =
            doc.engine.as_ref().and_then(|engine| engine.symfony_name_usages(kind, &named.name, include, cancel));
        let mut found: Vec<(PathBuf, e::Range)> = match known {
            Some(Ok(usages)) => usages.into_iter().map(|usage| (usage.path, usage.range)).collect(),
            Some(Err(error)) => return Err(engine_error(error)),
            // Elsewhere: the templates naming a template.
            None if kind == SYMFONY_TEMPLATE => {
                let mut found = phoxite::twig::template_usages(&named.name, &project.paths);
                if include && let Some(file) = project.paths.resolve(&named.name) {
                    found.push((file, e::Range::default()));
                }
                found
            }
            None => Vec::new(),
        };
        // The document as it is now, not as it was saved.
        let here = dunce::simplified(&doc.path).to_path_buf();
        found.retain(|(path, range)| dunce::simplified(path) != here || *range == e::Range::default());
        found.extend(
            twig_names(&doc.text, kind)
                .into_iter()
                .filter(|other| other.name == named.name)
                .map(|other| (doc.path.clone(), other.range)),
        );
        found.sort_by(|a, b| (&a.0, a.1.start).cmp(&(&b.0, b.1.start)));
        found.dedup();
        return Ok(Some(locations(found)));
    }
    let Some(engine) = doc.engine.clone() else { return Ok(None) };
    let found = match doc.kind {
        Kind::Php => engine.references(doc.id, at, include, cancel),
        Kind::Blade => return blade_references(&engine, &doc, at, include, cancel),
        Kind::Env => engine.laravel_env_references(&doc.path, &doc.text, at, include, cancel),
        Kind::TranslationJson => engine.laravel_translation_json_references(&doc.path, &doc.text, at, include, cancel),
        Kind::WordPressScript => engine.wordpress_script_references(&doc.path, &doc.text, at, include, cancel),
        Kind::Twig | Kind::Other => None,
    };
    match found {
        Some(Ok(found)) => Ok(Some(usages(found))),
        Some(Err(e)) => Err(engine_error(e)),
        None => Ok(None),
    }
}

/// A section, stack or component name; a view named at the caret; the
/// template's PHP; else the template's own view.
fn blade_references(
    engine: &PhpEngine,
    doc: &DocSnapshot,
    at: e::Pos,
    include: bool,
    cancel: &CancellationToken,
) -> Result<Option<Vec<lsp::Location>>> {
    if let Some(found) = engine.blade_name_usages(&doc.blade(), at, include) {
        return Ok(Some(usages(found)));
    }
    let name = match view_ref_at(&doc.text, at) {
        Some((ViewRef::View(name), _)) => name,
        Some(_) => return Ok(None),
        None if compile(&doc.text, false).to_php(at).is_some() => {
            return match engine.blade_references(&doc.blade(), at, include, cancel) {
                Some(Ok(found)) => Ok(Some(usages(found))),
                Some(Err(e)) => Err(engine_error(e)),
                None => Ok(None),
            };
        }
        None => match engine.laravel_view_name(&doc.path) {
            Some(name) => name,
            None => return Ok(None),
        },
    };
    let found = match engine.laravel_view_usages(&name, cancel) {
        Some(Ok(found)) => found,
        Some(Err(e)) => return Err(engine_error(e)),
        None => return Ok(None),
    };
    let mut out = Vec::new();
    if include && let Some(view) = engine.laravel_views().and_then(|views| views.get(&name).cloned()) {
        out.extend(location(&view.path, e::Range::default()));
    }
    out.extend(usages(found));
    Ok(Some(out))
}

pub fn document_highlight(
    state: &State,
    p: lsp::DocumentHighlightParams,
) -> Result<Option<Vec<lsp::DocumentHighlight>>> {
    let doc = snapshot(state, &p.text_document_position_params.text_document.uri)?;
    if doc.kind != Kind::Php {
        return Ok(None);
    }
    let Some(engine) = &doc.engine else { return Ok(None) };
    let found = engine.string_highlights(doc.id, pos(p.text_document_position_params.position));
    Ok(found.map(|found| {
        found
            .into_iter()
            .map(|h| lsp::DocumentHighlight {
                range: lsp_range(h.range),
                kind: Some(if h.write { lsp::DocumentHighlightKind::WRITE } else { lsp::DocumentHighlightKind::READ }),
            })
            .collect()
    }))
}

pub fn document_symbols(state: &State, p: lsp::DocumentSymbolParams) -> Result<Option<lsp::DocumentSymbolResponse>> {
    let doc = snapshot(state, &p.text_document.uri)?;
    let items = match (doc.kind, &doc.engine) {
        (Kind::Php, Some(engine)) => engine.document_symbols(doc.id),
        (Kind::Blade, Some(engine)) => Some(engine.blade_outline(&doc.text)),
        (Kind::Blade, None) => Some(phoxite::blade::structure::outline(&doc.text, &[])),
        (Kind::Twig, _) => Some(phoxite::twig::outline(&doc.text)),
        _ => None,
    };
    Ok(items
        .map(|items| lsp::DocumentSymbolResponse::Nested(items.into_iter().map(convert::document_symbol).collect())))
}

/// The most symbols one query returns from each folder.
const WORKSPACE_SYMBOLS: usize = 200;

#[allow(deprecated)]
pub fn workspace_symbols(
    state: &State,
    p: lsp::WorkspaceSymbolParams,
    cancel: &CancellationToken,
) -> Result<Option<lsp::WorkspaceSymbolResponse>> {
    let mut out = Vec::new();
    for engine in state.engines() {
        let found = engine.workspace_symbols(&p.query, WORKSPACE_SYMBOLS, cancel).map_err(engine_error)?;
        out.extend(found.into_iter().filter_map(|s| {
            Some(lsp::SymbolInformation {
                name: s.name,
                kind: convert::symbol_kind(s.kind),
                tags: None,
                deprecated: None,
                location: location(&s.path, s.range)?,
                container_name: s.container,
            })
        }));
    }
    Ok(Some(lsp::WorkspaceSymbolResponse::Flat(out)))
}

// ---- rename ----

pub fn prepare_rename(state: &State, p: lsp::TextDocumentPositionParams) -> Result<Option<lsp::PrepareRenameResponse>> {
    let doc = snapshot(state, &p.text_document.uri)?;
    let at = pos(p.position);
    if doc.kind == Kind::Twig {
        // A route or a template the project can rename, in PHP too.
        if let Some((kind, named)) = twig_name_at(&doc.text, at) {
            let renames = doc.engine.as_ref().is_some_and(|engine| engine.symfony_name_renames(kind, &named.name));
            return match renames {
                true => Ok(Some(lsp::PrepareRenameResponse::Range(lsp_range(named.range)))),
                false => Err("Nothing to rename here".into()),
            };
        }
        let project = state.twig_project(&doc.root);
        return match phoxite::twig::prepare_rename(&doc.text, at, &doc.path, &project.paths) {
            Some(range) => Ok(Some(lsp::PrepareRenameResponse::Range(lsp_range(range)))),
            None => Err("Nothing to rename here".into()),
        };
    }
    let Some(engine) = &doc.engine else { return Ok(None) };
    let found = match doc.kind {
        Kind::Php => engine.prepare_rename(doc.id, at).flatten(),
        Kind::Blade => engine.blade_prepare_rename(&doc.blade(), at).flatten(),
        Kind::Env => engine.laravel_env_prepare_rename(&doc.path, &doc.text, at),
        Kind::TranslationJson => engine.laravel_translation_json_prepare_rename(&doc.path, &doc.text, at),
        Kind::WordPressScript => engine.wordpress_script_prepare_rename(&doc.path, &doc.text, at).flatten(),
        Kind::Twig | Kind::Other => None,
    };
    match found {
        Some(range) => Ok(Some(lsp::PrepareRenameResponse::Range(lsp_range(range)))),
        None => Err("Nothing to rename here".into()),
    }
}

pub fn rename(state: &State, p: lsp::RenameParams, cancel: &CancellationToken) -> Result<Option<lsp::WorkspaceEdit>> {
    let doc = snapshot(state, &p.text_document_position.text_document.uri)?;
    let at = pos(p.text_document_position.position);
    if doc.kind == Kind::Twig {
        // A route or a template: the project renames it everywhere, from what is saved.
        if let Some((kind, named)) = twig_name_at(&doc.text, at) {
            let engine = doc.engine.as_ref();
            return match engine.and_then(|engine| engine.symfony_name_rename(kind, &named.name, &p.new_name, cancel)) {
                Some(Ok(edit)) => Ok(Some(convert::workspace_edit(edit, state.support.edits))),
                Some(Err(error)) => Err(engine_error(error)),
                None => Err("Nothing to rename here".into()),
            };
        }
        let project = state.twig_project(&doc.root);
        let found = phoxite::twig::rename(&doc.text, at, &doc.path, &p.new_name, &project.paths);
        let files = match found {
            Some(Ok(files)) => files,
            Some(Err(message)) => return Err(message),
            None => return Err("Nothing to rename here".into()),
        };
        let mut edit = e::WorkspaceEdit::default();
        for (path, ranges) in files {
            let edits = ranges.into_iter().map(|range| e::TextEdit { range, new_text: p.new_name.clone() }).collect();
            // The document as the editor has it; the other templates as they are on disk.
            let version = (dunce::simplified(&path) == dunce::simplified(&doc.path)).then_some(doc.version);
            edit.files.push(e::FileEdit::Edit { path, edits, version });
        }
        return Ok(Some(convert::workspace_edit(edit, state.support.edits)));
    }
    let Some(engine) = &doc.engine else { return Ok(None) };
    let new = p.new_name.as_str();
    let found = match doc.kind {
        Kind::Php => engine.rename_with_possible_usages(doc.id, at, new, cancel).map(|r| r.map(|outcome| outcome.edit)),
        Kind::Blade => engine.blade_rename(&doc.blade(), at, new, cancel),
        Kind::Env => engine.laravel_env_rename(&doc.path, &doc.text, at, new, cancel),
        Kind::TranslationJson => engine.laravel_translation_json_rename(&doc.path, &doc.text, at, new, cancel),
        Kind::WordPressScript => engine.wordpress_script_rename(&doc.path, &doc.text, at, new, cancel),
        Kind::Twig | Kind::Other => None,
    };
    match found {
        Some(Ok(edit)) => Ok(Some(convert::workspace_edit(edit, state.support.edits))),
        Some(Err(e)) => Err(engine_error(e)),
        None => Err("Nothing to rename here".into()),
    }
}

/// Moving or renaming PHP files and folders updates namespaces, references
/// and imports.
pub fn will_rename_files(
    state: &State,
    p: lsp::RenameFilesParams,
    cancel: &CancellationToken,
) -> Result<Option<lsp::WorkspaceEdit>> {
    let moves: Vec<(PathBuf, PathBuf)> = p
        .files
        .iter()
        .filter_map(|f| {
            let from = convert::path_of(&Url::parse(&f.old_uri).ok()?)?;
            let to = convert::path_of(&Url::parse(&f.new_uri).ok()?)?;
            Some((from, to))
        })
        .collect();
    let Some((from, _)) = moves.first() else { return Ok(None) };
    let Some((_, engine)) = state.workspace_of(from) else { return Ok(None) };
    if !moves.iter().any(|(from, _)| from.is_dir() || phoxite::project::is_php_path(from)) {
        return Ok(None);
    }
    let edit = engine.file_moves_edit(&moves, cancel).map_err(engine_error)?;
    Ok((!edit.files.is_empty()).then(|| convert::workspace_edit(edit, state.support.edits)))
}

// ---- code actions ----

fn code_action(state: &State, action: ImportAction) -> lsp::CodeActionOrCommand {
    lsp::CodeActionOrCommand::CodeAction(lsp::CodeAction {
        title: action.title,
        kind: Some(lsp::CodeActionKind::from(action.kind.to_owned())),
        is_preferred: action.preferred.then_some(true),
        edit: Some(convert::workspace_edit(action.edit, state.support.edits)),
        command: action.open.and_then(|open| {
            let target = location(&open.path, open.range)?;
            Some(lsp::Command {
                title: "Open".into(),
                command: crate::commands::OPEN_LOCATION.into(),
                arguments: Some(vec![serde_json::to_value(target).ok()?]),
            })
        }),
        ..lsp::CodeAction::default()
    })
}

pub fn code_actions(state: &State, p: lsp::CodeActionParams) -> Result<Option<lsp::CodeActionResponse>> {
    let doc = snapshot(state, &p.text_document.uri)?;
    let at = range(p.range);
    let only: Vec<String> =
        p.context.only.unwrap_or_default().into_iter().map(|kind| kind.as_str().to_owned()).collect();
    let wants = |kind: &str| {
        only.is_empty()
            || only.iter().any(|o| kind == o || kind.strip_prefix(o.as_str()).is_some_and(|r| r.starts_with('.')))
    };
    let mut actions = Vec::new();
    match (doc.kind, &doc.engine) {
        (Kind::Php, Some(engine)) => actions.extend(engine.code_actions(doc.id, at, &only).unwrap_or_default()),
        (Kind::Blade, engine) => {
            if wants(phoxite::imports::QUICKFIX)
                && let Some(engine) = engine
            {
                actions.extend(engine.blade_quick_fixes(&doc.blade(), at));
            }
            if wants(phoxite::imports::REWRITE) {
                actions.extend(phoxite::blade::intentions::code_actions(&doc.path, &doc.text, at));
            }
        }
        _ => return Ok(None),
    }
    // The refactorings with a dialog, for a client that has the dialogs.
    let dialogs = match (doc.kind, &doc.engine) {
        (Kind::Php, Some(engine)) if state.support.dialogs => {
            crate::refactor::dialog_actions(engine, &p.text_document.uri, doc.id, at, &only, &mut actions)
        }
        _ => Vec::new(),
    };
    let mut actions: Vec<_> = actions.into_iter().map(|action| code_action(state, action)).collect();
    actions.extend(dialogs);
    // Laravel: "Create translation key…" on a key some locale lacks.
    if wants(phoxite::imports::REWRITE)
        && let Some(engine) = &doc.engine
    {
        let request = match doc.kind {
            Kind::Php => engine.laravel_translation_request(doc.id, at.start),
            Kind::Blade => engine.blade_translation_request(&doc.blade(), at.start),
            _ => None,
        };
        actions.extend(request.as_ref().and_then(crate::commands::create_translation));
    }
    Ok(Some(actions))
}

// ---- formatting ----

pub fn format(
    state: &State,
    uri: &Url,
    within: Option<lsp::Range>,
    options: &lsp::FormattingOptions,
) -> Result<Option<Vec<lsp::TextEdit>>> {
    let doc = snapshot(state, uri)?;
    let line_width = state.settings_for(&doc.path).format.line_width;
    let within = within.map(range);
    let edits = match doc.kind {
        Kind::Php => {
            let Some(engine) = &doc.engine else { return Ok(None) };
            // The whole file, by the project's Pint when that is the formatter and it can run.
            if within.is_none()
                && let Some(formatted) = crate::pint::format(state, &doc)
            {
                if formatted == *doc.text {
                    return Ok(Some(Vec::new()));
                }
                // Pint adds and removes lines as it likes: the whole text is replaced.
                let last = doc.text.rsplit('\n').next().unwrap_or_default();
                let end = lsp::Position::new(doc.text.matches('\n').count() as u32, last.encode_utf16().count() as u32);
                let whole = lsp::Range::new(lsp::Position::new(0, 0), end);
                return Ok(Some(vec![lsp::TextEdit::new(whole, formatted)]));
            }
            let options = phoxite::format::FormatOptions {
                indent: options.tab_size,
                use_tabs: !options.insert_spaces,
                line_width,
            };
            engine.format(doc.id, within, &options)
        }
        Kind::Blade => {
            use phoxite::blade::format::{BladeFormatOptions, edits, format};
            let php = Some(doc.engine.as_ref().map_or(phoxite::stubs::PhpVersion::LATEST, |e| e.language_level()));
            let options =
                BladeFormatOptions { indent: options.tab_size, use_tabs: !options.insert_spaces, line_width, php };
            Some(match &doc.engine {
                Some(engine) => engine.blade_format(&doc.text, within, &options),
                None => edits(&doc.text, &format(&doc.text, &options, &[]), within.map(|r| (r.start.line, r.end.line))),
            })
        }
        Kind::Twig => {
            // The whole template, by the project's Prettier when that is the formatter and it can run.
            if within.is_none()
                && let Some(formatted) = crate::prettier::format(state, &doc)
            {
                if formatted == *doc.text {
                    return Ok(Some(Vec::new()));
                }
                // Prettier rewraps as it likes: the whole text is replaced.
                let last = doc.text.rsplit('\n').next().unwrap_or_default();
                let end = lsp::Position::new(doc.text.matches('\n').count() as u32, last.encode_utf16().count() as u32);
                let whole = lsp::Range::new(lsp::Position::new(0, 0), end);
                return Ok(Some(vec![lsp::TextEdit::new(whole, formatted)]));
            }
            let options =
                phoxite::twig::TwigFormatOptions { indent: options.tab_size, use_tabs: !options.insert_spaces };
            Some(phoxite::twig::format(&doc.text, &options, within.map(|r| (r.start.line, r.end.line))))
        }
        _ => None,
    };
    Ok(edits.map(|edits| edits.into_iter().map(convert::text_edit).collect()))
}

// ---- hints, lenses, tokens ----

pub fn inlay_hints(state: &State, p: lsp::InlayHintParams) -> Result<Option<Vec<lsp::InlayHint>>> {
    use phoxite::signature::InlayHintKind;
    let doc = snapshot(state, &p.text_document.uri)?;
    let (Kind::Php, Some(engine)) = (doc.kind, &doc.engine) else { return Ok(None) };
    let within = range(p.range);
    let mut hints: Vec<lsp::InlayHint> = engine
        .inlay_hints(doc.id, within)
        .unwrap_or_default()
        .into_iter()
        .map(|hint| lsp::InlayHint {
            position: lsp_pos(hint.position),
            label: lsp::InlayHintLabel::String(hint.label),
            kind: Some(match hint.kind {
                InlayHintKind::Parameter => lsp::InlayHintKind::PARAMETER,
                InlayHintKind::Type => lsp::InlayHintKind::TYPE,
            }),
            text_edits: None,
            tooltip: hint.tooltip.map(lsp::InlayHintTooltip::String),
            padding_left: Some(hint.padding_left),
            padding_right: Some(hint.padding_right),
            data: None,
        })
        .collect();
    // Laravel: the full URI after each route registration.
    for hint in engine.laravel_route_hints(doc.id, within).unwrap_or_default() {
        hints.push(lsp::InlayHint {
            position: lsp_pos(hint.position),
            label: lsp::InlayHintLabel::LabelParts(vec![lsp::InlayHintLabelPart {
                value: hint.label,
                tooltip: None,
                location: None,
                command: crate::commands::route_actions(String::new(), &hint.routes),
            }]),
            kind: None,
            text_edits: None,
            tooltip: None,
            padding_left: Some(true),
            padding_right: Some(false),
            data: None,
        });
    }
    hints.sort_by_key(|hint| (hint.position.line, hint.position.character));
    Ok(Some(hints))
}

/// Whether two paths name one file (Windows: whatever the case).
fn same_file(a: &Path, b: &Path) -> bool {
    let (a, b) = (dunce::simplified(a), dunce::simplified(b));
    a == b || (cfg!(windows) && a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy()))
}

/// Laravel: the routes of each controller method, the model view of each
/// Eloquent model, and running each console command.
pub fn code_lens(state: &State, p: lsp::CodeLensParams) -> Result<Option<Vec<lsp::CodeLens>>> {
    let doc = snapshot(state, &p.text_document.uri)?;
    let (Kind::Php, Some(engine)) = (doc.kind, &doc.engine) else { return Ok(None) };
    let mut lenses: Vec<lsp::CodeLens> = engine
        .laravel_route_lenses(doc.id)
        .unwrap_or_default()
        .into_iter()
        .map(|lens| lsp::CodeLens {
            range: lsp_range(lens.range),
            command: crate::commands::route_actions(lens.title(), &lens.routes),
            data: None,
        })
        .collect();
    lenses.extend(engine.laravel_model_lenses(doc.id).unwrap_or_default().into_iter().map(|lens| lsp::CodeLens {
        range: lsp_range(lens.range),
        command: Some(lsp::Command {
            title: lens.title,
            command: phoxite::frameworks::laravel::model_view::MODEL_VIEW_COMMAND.to_owned(),
            arguments: Some(vec![Value::String(lens.class)]),
        }),
        data: None,
    }));
    if let Some(artisan) = engine.laravel_artisan() {
        for command in &artisan.commands {
            if let (Some(path), Some(range)) = (&command.path, command.class_range)
                && same_file(path, &doc.path)
            {
                lenses.push(lsp::CodeLens {
                    range: lsp_range(range),
                    command: Some(crate::commands::run_artisan(&command.name)),
                    data: None,
                });
            }
        }
    }
    lenses.sort_by_key(|lens| (lens.range.start.line, lens.range.start.character));
    Ok(Some(lenses))
}

pub fn semantic_tokens_legend() -> lsp::SemanticTokensLegend {
    use phoxite::semantic::{TOKEN_MODIFIERS, TOKEN_TYPES};
    lsp::SemanticTokensLegend {
        token_types: TOKEN_TYPES.iter().map(|t| lsp::SemanticTokenType::new(t)).collect(),
        token_modifiers: TOKEN_MODIFIERS.iter().map(|m| lsp::SemanticTokenModifier::new(m)).collect(),
    }
}

pub fn semantic_tokens(state: &State, p: lsp::SemanticTokensParams) -> Result<Option<lsp::SemanticTokensResult>> {
    let doc = snapshot(state, &p.text_document.uri)?;
    let tokens = match (doc.kind, &doc.engine) {
        (Kind::Twig, _) => phoxite::twig::semantic_tokens(&doc.text),
        (Kind::Php, Some(engine)) => match engine.semantic_tokens(doc.id) {
            Some(tokens) => tokens,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    let tokens = tokens.into_iter().map(|t| (t.start.line, t.start.col, t.len, t.ty.index(), t.modifiers)).collect();
    Ok(Some(lsp::SemanticTokensResult::Tokens(lsp::SemanticTokens {
        result_id: None,
        data: convert::semantic_tokens(tokens),
    })))
}

// ---- type and call hierarchies ----

/// Where a hierarchy item points: what its `data` holds.
#[derive(Serialize, Deserialize)]
struct ItemAt {
    path: PathBuf,
    pos: e::Pos,
}

/// Runs `f` on the document of `path`: the open one, or the file opened for
/// the call and closed after it.
fn with_document<T>(state: &State, path: &Path, f: impl FnOnce(&PhpEngine, e::DocId) -> T) -> Option<T> {
    let (_, engine) = state.workspace_of(path)?;
    if let Some(uri) = convert::uri_of(path)
        && let Some(doc) = state.snapshot(&uri)
        && doc.kind == Kind::Php
    {
        return Some(f(&engine, doc.id));
    }
    let text = std::fs::read_to_string(path).ok()?;
    let id = e::DocId(u32::MAX - (blake3::hash(path.to_string_lossy().as_bytes()).as_bytes()[0] as u32));
    engine.document_opened(id, path, 0, &text);
    let out = f(&engine, id);
    engine.document_closed(id);
    Some(out)
}

/// The declaration a hierarchy starts from: the definition of the name at
/// the caret, as its file's outline describes it.
fn hierarchy_root(state: &State, uri: &Url, at: lsp::Position) -> Result<Option<HierarchyEntry>> {
    let doc = snapshot(state, uri)?;
    let (Kind::Php, Some(engine)) = (doc.kind, &doc.engine) else { return Ok(None) };
    let at = pos(at);
    let target = engine
        .definition(doc.id, at)
        .and_then(|found| found.into_iter().next())
        .unwrap_or(Target { path: doc.path.clone(), range: e::Range::point(at) });
    let found = with_document(state, &target.path, |engine, id| {
        let outline = engine.document_symbols(id)?;
        innermost(&outline, target.range.start).map(|item| HierarchyEntry {
            name: item.name.clone(),
            kind: item.kind,
            detail: item.detail.clone(),
            path: target.path.clone(),
            range: item.range,
            selection_range: item.selection_range,
            from_ranges: Vec::new(),
        })
    });
    Ok(found.flatten())
}

fn innermost(items: &[e::OutlineItem], at: e::Pos) -> Option<&e::OutlineItem> {
    let item = items.iter().find(|item| item.range.start <= at && at <= item.range.end)?;
    innermost(&item.children, at).or(Some(item))
}

fn related(
    state: &State,
    data: Option<Value>,
    direction: Direction,
    calls: bool,
    cancel: &CancellationToken,
) -> Result<Vec<HierarchyEntry>> {
    let Some(at) = data.and_then(|data| serde_json::from_value::<ItemAt>(data).ok()) else { return Ok(Vec::new()) };
    let found = with_document(state, &at.path, |engine, id| {
        if calls {
            engine.call_hierarchy(id, at.pos, direction, cancel)
        } else {
            engine.type_hierarchy(id, at.pos, direction)
        }
    });
    match found.flatten() {
        Some(Ok(found)) => Ok(found),
        Some(Err(e)) => Err(engine_error(e)),
        None => Ok(Vec::new()),
    }
}

fn item_data(entry: &HierarchyEntry) -> Option<Value> {
    serde_json::to_value(ItemAt { path: entry.path.clone(), pos: entry.selection_range.start }).ok()
}

fn call_item(entry: &HierarchyEntry) -> Option<lsp::CallHierarchyItem> {
    Some(lsp::CallHierarchyItem {
        name: entry.name.clone(),
        kind: convert::symbol_kind(entry.kind),
        tags: None,
        detail: entry.detail.clone(),
        uri: convert::uri_of(&entry.path)?,
        range: lsp_range(entry.range),
        selection_range: lsp_range(entry.selection_range),
        data: item_data(entry),
    })
}

fn type_item(entry: &HierarchyEntry) -> Option<lsp::TypeHierarchyItem> {
    Some(lsp::TypeHierarchyItem {
        name: entry.name.clone(),
        kind: convert::symbol_kind(entry.kind),
        tags: None,
        detail: entry.detail.clone(),
        uri: convert::uri_of(&entry.path)?,
        range: lsp_range(entry.range),
        selection_range: lsp_range(entry.selection_range),
        data: item_data(entry),
    })
}

pub fn prepare_call_hierarchy(
    state: &State,
    p: lsp::CallHierarchyPrepareParams,
) -> Result<Option<Vec<lsp::CallHierarchyItem>>> {
    let p = p.text_document_position_params;
    Ok(hierarchy_root(state, &p.text_document.uri, p.position)?
        .and_then(|entry| call_item(&entry))
        .map(|item| vec![item]))
}

pub fn incoming_calls(
    state: &State,
    p: lsp::CallHierarchyIncomingCallsParams,
    cancel: &CancellationToken,
) -> Result<Option<Vec<lsp::CallHierarchyIncomingCall>>> {
    let found = related(state, p.item.data, Direction::Up, true, cancel)?;
    Ok(Some(
        found
            .iter()
            .filter_map(|entry| {
                Some(lsp::CallHierarchyIncomingCall {
                    from: call_item(entry)?,
                    from_ranges: entry.from_ranges.iter().copied().map(lsp_range).collect(),
                })
            })
            .collect(),
    ))
}

pub fn outgoing_calls(
    state: &State,
    p: lsp::CallHierarchyOutgoingCallsParams,
    cancel: &CancellationToken,
) -> Result<Option<Vec<lsp::CallHierarchyOutgoingCall>>> {
    let found = related(state, p.item.data, Direction::Down, true, cancel)?;
    Ok(Some(
        found
            .iter()
            .filter_map(|entry| {
                Some(lsp::CallHierarchyOutgoingCall {
                    to: call_item(entry)?,
                    from_ranges: entry.from_ranges.iter().copied().map(lsp_range).collect(),
                })
            })
            .collect(),
    ))
}

pub fn prepare_type_hierarchy(
    state: &State,
    p: lsp::TypeHierarchyPrepareParams,
) -> Result<Option<Vec<lsp::TypeHierarchyItem>>> {
    let p = p.text_document_position_params;
    Ok(hierarchy_root(state, &p.text_document.uri, p.position)?
        .and_then(|entry| type_item(&entry))
        .map(|item| vec![item]))
}

pub fn type_relatives(
    state: &State,
    data: Option<Value>,
    direction: Direction,
    cancel: &CancellationToken,
) -> Result<Option<Vec<lsp::TypeHierarchyItem>>> {
    let found = related(state, data, direction, false, cancel)?;
    Ok(Some(found.iter().filter_map(type_item).collect()))
}
