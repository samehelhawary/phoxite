# Framework support: plan and progress

Tracks the plan to support what Phoxite doesn't cover yet: Twig in the server and VS Code,
deeper Symfony, and WordPress, Drupal, CodeIgniter, Yii, CakePHP, Laminas and Magento.
How the pieces fit is in [frameworks.md](frameworks.md).

Depth for each new framework: types, plus completion, goto, usages, rename and an
undefined-name inspection for the strings that name things. No tool views in VS Code.

Last updated: 2026-11-20.

## Status

| Phase | What | Status |
| --- | --- | --- |
| 0 | Framework-neutral foundations | Done |
| 1 | Twig, end to end | Done |
| 2 | Symfony and Doctrine, deeper | Done |
| 2.5 | Callable-string references | Not started (next) |
| 3 | WordPress | Not started |
| 4 | Drupal | Not started |
| 5 | CodeIgniter 4 | Not started |
| 6 | Yii 2 | Not started |
| 7 | CakePHP 4 and 5 | Not started |
| 8 | Laminas (MVC and Mezzio) | Not started |
| 9 | Magento 2 | Not started |

Order: 0, then 1 → 2 → 4. Phase 2.5 comes before phase 3. Phases 3 and 5–8 need only phase
0 and can be reordered. Phase 9 is last.

## Decisions

- [x] Three foundation pieces are built with their first users, not in phase 0, so that a
      real case shapes each one: the generic name facts with WordPress hooks (phase 3), the
      callable-string references as a change of their own before phase 3 (phase 2.5), and
      the PHP-array reader with Yii (phase 6; Laminas, phase 8, needs it too).

- [x] Phase 0 is committed on branch `framework-foundations`.

## Phase 0 — Foundations

- [x] **0.1 Detection.** `frameworks/detect.rs` (`Framework`, `Frameworks`,
      `ComposerPackages`), `PhpEngine::frameworks()`, the `frameworks` setting in the
      engine, the server, `package.json` and the schema. `frameworks.laravel` wins over
      `laravel.detection` unless it is `auto`.
- [x] **0.2 Shared string references.** The machinery is `frameworks/refs/`; Laravel's 24
      kinds stay in `frameworks/laravel/refs/` and register through `kinds_for`. The
      inspection list in `inspect/laravel.rs` is per framework.
- **0.3 Generic name facts:** moved to phase 3.
- **0.4 Callable-string references:** moved to phase 2.5.
- [x] **0.5a YAML reader** moved to `base/yaml.rs` (re-exported from `symfony`).
- **0.5b PHP-array reader:** moved to phase 6.
- [x] **0.6 Symfony routes as references.** `symfonyRoute` kind
      (`frameworks/symfony/refs/route_name.rs`): goto, usages, rename and hover;
      `TypeProvider::known_strings` removed.
- [x] **0.7 Docs.** `frameworks.md`, the stale lines of `architecture.md`, changelogs,
      READMEs.

Also done in phase 0:

- [x] Strings inside attributes (`#[Route('/x', name: 'home')]`) are recognised as strings
      (`complete::string_start` read `#[` as a comment).

Left over from phase 0:

- [x] Run the VS Code end-to-end suite: 31 checks in one project and 2 in two projects
      pass (2026-11-20, VS Code 1.147.0). (No by-hand check for this phase: the Symfony
      support is all in the server and the extension adds no client code for it, so the
      server tests, which send the requests VS Code sends, cover goto and rename on a route
      name. By-hand checks start with phase 1, where highlighting has to be looked at.)
- [ ] Make the helper-instance table of `frameworks/refs` per framework. It says `view()`,
      `config()` and others return a Laravel service, in every project; CodeIgniter has
      global functions of the same names. Do it when phase 5 starts.

Left as they are: the `ide.json` rules and the Laravel kinds named in
`undefined_references` also live in `frameworks/refs`, and do nothing outside a Laravel
project. Move them only if they get in the way.

## Phase 1 — Twig, end to end

Engine (`crates/phoxite/src/twig/`):

- [x] `problems(text)`: the blocks left open and what the parser reports, each broken tag
      blanked in turn so the errors after it are found. A tag Twig doesn't have is not an
      error (extensions add tags).
- [x] `hover(text, pos, symfony)`: the built-ins and the macros in scope.
- [x] `definition`: template names, macro calls (`forms.input()`, `_self.input()`, names of
      `from … import`), the block a block overrides in a parent. `template_usages`.
- [x] Template names are listed once per `TemplatePaths`; `TwigProject` (the paths and
      whether Symfony's bridge is installed) is kept by the server, not the engine, since
      Twig needs no engine, and dropped when a template, `twig.yaml` or a Composer file
      changes.

Server (`crates/phoxite-lsp/src/`):

- [x] `Kind::Twig` in `state.rs` (language id `twig`, or a name ending in `.twig`).
- [x] Twig problems in `template_problems`, source `twig`, code `TwigSyntax`.
- [x] Twig branches in `handlers.rs`: completion, hover, goto, references, document symbols.
- [x] `|` as a completion trigger character (answered in Twig only); `**/*.twig` and
      `**/config/**/*.{yaml,yml}` in `WATCHED_FILES`. The YAML glob is wider than planned:
      Symfony's routes and services files weren't watched at all.
- [x] `phoxite-lsp check` reports Twig files.

VS Code (`editors/vscode/`):

- [x] `twig` language, grammar, snippets and `onLanguage:twig` in `package.json`.
- [x] `syntaxes/twig.tmLanguage.json`, `languages/twig.language-configuration.json` and
      `snippets/twig.json`, written for this project; credit in `THIRD_PARTY_LICENSES.md`.
- [x] `twig` in the status item and the document selector (`src/extension.ts`).
- [x] A `twig` overlap in `src/conflicts.ts`: `mblode.twig-language-2` and
      `mblode.twig-language`, both found on the Marketplace. `whatwedo.twig` was not
      confirmed and is left out.
- [x] Grammar tests (`test/twig-grammar.test.mts`) and an end-to-end check.

Docs:

- [x] `docs/editors.md`: Twig for Neovim, Helix, Zed and Sublime.
- [x] `docs/openidea.md`: OpenIDEA's own Twig service and how to avoid doubled answers.
- [x] `architecture.md`: the Twig line.
- [x] README and the changelog.

Added after the first pass (they were this phase's leftovers):

- [x] Highlighting looked at in VS Code 1.147.0 (2026-11-20): tags, strings, filters,
      numbers and comments are coloured in text, inside attribute values and in script and
      style blocks; a `verbatim` body is plain; the HTML parts match a plain HTML file in
      the same theme.
- [x] Twig 1 and 2: the parser accepts what was tried (`{% for … if … %}`, `spaceless`,
      `filter`, `sameas`, `divisibleby`), so no error is known to be wrong there and
      nothing depends on the Twig version. `"TwigSyntax": { "enabled": false }` under
      `inspections` turns the errors off (server and `check`).
- [x] Rename (`twig/rename.rs`): a block in its `extends`/`use` family; a macro where it is
      declared, imported and called.
- [x] Formatting (`twig/format.rs`): the indentation of every line from the HTML elements
      and Twig blocks around it, and the spacing inside tags per Twig's coding standards.
      Only whitespace changes; nothing is rewrapped.
- [x] A block inside `{% embed 'x' %}` is a block of `x`: rename takes the family of `x`,
      and goto goes to the block of `x`.
- [x] Formatting: the elements every branch of an `{% if %}` … `{% else %}` opens are open
      after `{% endif %}`, so what follows nests in them until their closing tags. An element
      opened in one block and closed in another doesn't nest.
- [x] Formatting: the code of `<script>` and `<style>` is indented by its brackets, with a
      chained line (`.then(…)`, `&& …`) one level further. Where that isn't safe (a template
      literal, a string left open at the end of a line, brackets that don't balance) it
      keeps its own indentation, shifted as a whole.
- [x] Semantic tokens (`twig/tokens.rs`): filters, tests, functions, methods, properties
      and variables, with the legend PHP uses.

Left over from phase 1:

- Rename of a template's own name: moved to phase 2, right after "Template names in PHP".
  It depends on that one item only: until PHP's `render('x.html.twig')` is a reference, a
  rename would move the file and leave those calls pointing at nothing.
- [x] Decided and built: full formatting (the JavaScript and CSS themselves, rewrapped
      lines) is the project's Prettier's, not ours. `format.twig: "prettier"` runs
      `node_modules/prettier` on the template's text through `--stdin-filepath`, so the
      project's configuration and its Twig plugin apply (`crates/phoxite-lsp/src/prettier.rs`,
      modelled on `pint.rs`): trusted workspaces only, whole templates only, the built-in
      formatter when Prettier is missing or fails. `analyzers.nodePath` names the Node.js.
      The other option, embedding Rust formatters (`markup_fmt` with a JavaScript and a CSS
      formatter), would grow the binary and wouldn't match what a team's CI runs.
      Tested with a stand-in, and with Prettier 3 and `@zackad/prettier-plugin-twig` through
      the ignored test `a_real_prettier_formats_the_template` (`PHOXITE_REAL_PRETTIER`).

Nothing else of phase 1 is open.

## Phase 2 — Symfony and Doctrine, deeper

- [x] Route names in Twig's `path()` and `url()`: completion, goto, usages and rename from
      the template, and the templates count as usages of a route from PHP.
      (`ReferenceKind::other_usages` is the hook: usages in files the word index doesn't
      have.)
- [x] Template names in PHP (`symfonyTemplate`: `render()`, `renderView()`, `renderBlock()`,
      `renderBlockView()`, `renderForm()`, `stream()` on a controller; `render()`,
      `display()`, `load()`, `resolveTemplate()` on `Twig\Environment`) to the Twig file,
      with the templates naming it as usages.
- [x] Rename of a template's own name, from PHP or from a template: the file moves, and the
      name changes in the PHP strings and in the templates. A name with a `@Namespace` isn't
      renamed.
- [x] An attribute's arguments are string sites (its class's `__construct`):
      `#[Template('x.html.twig')]`, `#[Autowire(service: …)]`, `#[Autowire(param: …)]`.
- [x] `PhpUndefinedTemplate`: a name without a `@Namespace` that ends in `.twig` and isn't
      in the template folders of a project that has templates.
- [x] Usages and rename read a template an editor has open as the editor has it
      (`PhpEngine::set_template_text`, which the server calls for Twig documents).
- [x] Service ids (`symfonyService`: `get()` and `has()` on a container) and parameters
      (`symfonyParameter`: `getParameter()`, a parameter bag's `get()`), from the services
      files, whose `parameters:` are read now: completion, goto, usages, hover.
- [x] Service ids and parameters inside the YAML files below `config/` (`@mailer`,
      `@?mailer`, `%app.locale%`) are usages, and both are renamed. Not a service whose id
      is its class name.
- [x] Translation keys (`symfonyTranslation`: `trans()` on a translator, `new
      TranslatableMessage()`, `t()`, and `'key'|trans` in a template) from the YAML
      catalogues of `translations/` (`symfony/translations.rs`): completion, goto to every
      locale, usages, hover.
- [x] The project's Twig extensions: `new TwigFilter` / `TwigFunction` / `TwigTest` and
      `#[AsTwigFilter]` / `#[AsTwigFunction]` / `#[AsTwigTest]` are facts of the summary
      (`SymfonyFacts::twig`, schema 38); completion and goto in templates.
- [x] Variables a controller passes to a template (`frameworks/symfony/templates.rs`):
      completion of the variables with their types, and of the members after a `.` (public
      properties, methods, getters under the name Twig reads them by), through a chain and
      for a `{% for %}` loop's variable.
- [x] Doctrine: association properties typed from the ORM attributes
      (`SymfonyFacts::associations`); the entity's fields in the keys of `findBy()`,
      `findOneBy()` and `count()` (`doctrineField`).

Dropped:

- Symfony detection as the project flag for the type provider. The provider asks whether
  the `FrameworkBundle` class is known, which the resolver caches; a flag would save
  nothing and would need the resolver to know the project's frameworks.

Left over from phase 2:

- [x] Template members: hover with the type, and goto to the property or the getter Twig
      calls. (No warning for a member that doesn't exist: a variable's type is often not
      known, and Twig reads arrays the same way.)
- [x] XLIFF catalogues are read; translation keys complete before `|trans`.
- [x] Translation keys are renamed (the last part of a nested key, to a name without a
      dot; not a key some catalogues nest and others write whole), and JSON catalogues are
      read.
- [x] A service after `alias:` or `parent:` in YAML is a usage, and the files of
      `config/packages` are read for parameters and services like Symfony's kernel does.
- [x] The Twig names a package of `vendor/` adds are known: they are facts of indexed files
      like the project's own (checked by a test; nothing had to change).

Nothing else of phase 2 is open.

Moved to phase 6, after the PHP-array reader they need: Symfony's `services.php` and
`routes.php`, and its PHP translation catalogues.

## Phase 2.5 — Callable-string references

A change of its own: it is on in every PHP project, not only framework ones, and looks at
every call with a string argument, so a slowdown has to be traceable to it.

- [ ] A reference kind for `'my_function'` and `[Foo::class, 'method']` passed to a
      `callable` parameter: completion, goto, usages, rename. Builds on
      `navigate/references.rs` (`callable_arguments`) and `refactor/signature.rs`
      (`callable_strings`).
- [ ] A group of kinds that is on whatever the project's frameworks
      (`frameworks::refs::kinds_for`).
- [ ] Budget and bench runs before and after, on the fixture and on laravel/framework.

## Phase 3 — WordPress

Needs phase 2.5.

- [ ] First: the generic name facts. One summary struct for the names a framework declares
      and uses (`summary/names.rs`), one index key per kind, one resolver slot; shaped by
      hooks (interpolated names, callback targets, the arguments at the firing site).
- [ ] Hooks: declared by `do_action` / `apply_filters`, used by `add_action`, `add_filter`,
      `remove_*`, `has_*`, `did_action`, `doing_*`. Completion, goto, usages, rename;
      interpolated names matched by pattern; the undefined-hook inspection off by default.
- [ ] Callbacks of `add_action`, `add_filter`, `add_shortcode`, activation hooks, REST and
      AJAX callbacks (the phase 2.5 kind).
- [ ] Shortcode tags, post types and taxonomies, script and style handles, option names,
      image sizes, nav-menu locations, template parts.
- [ ] Parameter types of a hook callback from the hook's firing site.
- [ ] Types of `global $wpdb, $post, $wp_query…` from the `@global` tags of the indexed core.
- [ ] Remove the `wordpress` stub examples (`base/settings.rs`, `package.json`, the schema)
      and document the plugin-only setup (`includePaths` or a stubs package).

## Phase 4 — Drupal (8 and later)

Needs phases 1 and 2.

- [ ] Treat `.module`, `.install`, `.theme`, `.profile`, `.inc` and `.engine` as PHP in a
      Drupal project (engine, server, VS Code file associations).
- [ ] Service ids from `*.services.yml`: names and the types of `\Drupal::service('id')` and
      `$container->get('id')`.
- [ ] Route names from `*.routing.yml`: `Url::fromRoute()`, `Link::createFromRoute()`,
      `$this->redirect()`, `*.links.*.yml`.
- [ ] Hooks: `*.api.php` definitions, `<module>_<hook>()` implementations, `#[Hook]`
      methods, `invokeAll()` / `alter()` calls.
- [ ] Plugin ids from attributes and annotations to `createInstance('id')`.
- [ ] Entity type ids and their storage and entity classes (`getStorage('node')`).
- [ ] Permissions, library names, config names, theme hooks.
- [ ] Template folders of modules and themes in `TemplatePaths`.

## Phase 5 — CodeIgniter 4

- [ ] First: the helper-instance table per framework (see phase 0's leftovers).
- [ ] Types: `service('name')`, `Services::name()`, `model()`, `config()`,
      `single_service()`; functions a `helper()` call loads.
- [ ] Names: views, route names (`route_to()`, `url_to()`), language keys, filter aliases,
      helper names.

## Phase 6 — Yii 2

- [ ] First: the PHP-array reader. A reusable reader for PHP files that return an array,
      generalised from `frameworks/laravel/config.rs`, with `Foo::class` keys and
      class-constant values (Laminas needs those).
- [ ] Then, for Symfony (moved from phase 2, where only the reader was missing):
  - [ ] `config/services.php` and `config/routes.php` where they return plain arrays: their
        services, parameters and routes beside the YAML files' (`symfony/mod.rs`).
  - [ ] PHP translation catalogues (`translations/messages.en.php`): their keys beside the
        YAML and XLIFF ones (`symfony/translations.rs`).
- [ ] Types: `Yii::$app->…` components from the config arrays, `Yii::createObject()`,
      `Yii::$container->get()`, `ActiveQuery<Model>`, relation properties.
- [ ] Names: views, routes (`Url::to(['site/index'])`), path aliases, translation
      categories and messages, attribute names in `rules()` and `attributeLabels()`.

## Phase 7 — CakePHP 4 and 5

- [ ] Types: `fetchTable()` and `TableRegistry`, association properties, `loadComponent()`,
      view helpers, `$this` in templates, entity classes.
- [ ] Names: table aliases, component, helper and behaviour names, templates and elements,
      plugin dot notation, `Configure::read()` keys, route arrays.

## Phase 8 — Laminas (MVC and Mezzio)

Needs the PHP-array reader (phase 6; build it here if this phase comes first).

- [ ] Types: `$container->get()` through `aliases`, `factories` and `invokables`; controller
      plugins; view helpers in `.phtml` templates.
- [ ] Names: route names, template names, service names, config keys.

## Phase 9 — Magento 2

- [ ] An XML reader (`base/xml.rs`); choose between `quick-xml` and a small in-house one.
- [ ] Synthesised classes for generated code: `FooFactory`, `Foo\Proxy`, `Foo\Interceptor`,
      extension-attribute interfaces.
- [ ] Types: `di.xml` preferences and virtual types; `DataObject` magic accessors.
- [ ] Names: events, template ids, layout handles and block names, config paths, ACL
      resources, module names, plugin methods.

## Every framework phase (2–9) also delivers

- [ ] `frameworks/<name>/project.rs` and `frameworks/<name>/refs/`; a provider in
      `types/provider/<name>.rs`.
- [ ] `fixtures/<name>-app`, written for this repository, with a stubs fixture for `vendor/`.
- [ ] Engine tests and a server test.
- [ ] A corpus entry in `scripts/php-corpus.sh` and a differential run with no new false
      positives.
- [ ] The VS Code status names the framework.
- [ ] `docs/engine/<name>.md`, README, `editors/vscode/README.md`, `docs/editors.md`, both
      changelogs.

## Checks before a phase is done

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo test -p phoxite --release --test budgets -- --ignored`
- [ ] In `editors/vscode`: `npm run typecheck`, `npm run lint`, `npm test`,
      `npm run test:e2e`
- [ ] `phoxite-lsp check` on the phase's fixture and corpus project
- [ ] By hand in VS Code on the fixture, when the phase adds something to look at
      (highlighting, a status text) or client code

Phase 0: all pass (934 engine tests, the server tests, 50 extension unit tests, the
end-to-end suite, all budgets within limits). `phoxite-lsp check` on a corpus and the
by-hand check don't apply to it.
