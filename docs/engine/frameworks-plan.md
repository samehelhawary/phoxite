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
| 1 | Twig, end to end | Not started (next) |
| 2 | Symfony and Doctrine, deeper | Not started |
| 2.5 | Callable-string references | Not started |
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

- [ ] `problems(text)` from the parse error `recover.rs` locates.
- [ ] `hover(text, pos, symfony)` from `Builtin.doc`.
- [ ] Goto for macros and blocks; usages of a template name.
- [ ] Cache `TemplatePaths` in the engine, invalidated by watcher batches.

Server (`crates/phoxite-lsp/src/`):

- [ ] `Kind::Twig` in `state.rs` (language id `twig`, or a name ending in `.twig`).
- [ ] Twig problems in `template_problems`, source `twig`.
- [ ] Twig branches in `handlers.rs`: completion, hover, goto, references, document symbols;
      a `TwigCompletionKind` mapping.
- [ ] `|` as a completion trigger character; `**/*.twig` and
      `**/config/packages/twig.{yaml,yml}` in `WATCHED_FILES`.
- [ ] `phoxite-lsp check` reports Twig files.

VS Code (`editors/vscode/`):

- [ ] `twig` language, grammar, snippets and `onLanguage:twig` in `package.json`.
- [ ] `syntaxes/twig.tmLanguage.json` and `languages/twig.language-configuration.json`,
      written for this project; credit in `THIRD_PARTY_LICENSES.md`.
- [ ] `twig` in the status item and the document selector (`src/extension.ts`).
- [ ] A `twig` overlap in `src/conflicts.ts` (extension ids checked on the Marketplace).
- [ ] Grammar test and an end-to-end check.

Docs:

- [ ] `docs/editors.md`: a `twig` filetype for Neovim, Helix, Zed and Sublime.
- [ ] `docs/openidea.md`: OpenIDEA's own Twig service and how to avoid doubled answers.
- [ ] `architecture.md`: the Twig line (it says the server doesn't serve it yet).
- [ ] README and both changelogs.

## Phase 2 — Symfony and Doctrine, deeper

- [ ] Symfony detection as the project flag for the type provider (today the provider
      checks the `FrameworkBundle` class on each call).
- [ ] Route names in Twig's `path()` and `url()`.
- [ ] Template names in PHP (`render()`, `renderView()`, `renderBlock()`, `#[Template]`) to
      the Twig file, with usages from the Twig side.
- [ ] Service ids and parameters: `$container->get('id')`, `getParameter('x')`, `%x%` and
      `@id` in YAML, `#[Autowire]`.
- [ ] Translation keys: `trans('key')` and `|trans` to `translations/*.yaml`.
- [ ] The project's Twig extensions: filters, functions and tests from `AbstractExtension`
      and `#[AsTwigFilter]` / `#[AsTwigFunction]` / `#[AsTwigTest]`.
- [ ] Variables a controller passes to a template, typed in the template; member completion
      after `.` in Twig.
- [ ] Doctrine: association properties from the ORM attributes; field names in `findBy([…])`
      keys.
- [ ] `services.php` and `routes.php` where they are plain arrays. Waits for the PHP-array
      reader (phase 6), or builds it here if this is wanted sooner.

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
