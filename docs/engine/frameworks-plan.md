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
| 2.5 | Callable-string references | Done |
| 3 | WordPress | Done |
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

- [x] `'my_function'`, `'Foo::method'`, `[$o, 'method']` and `[Foo::class, 'method']`
      passed where only a callable is expected: goto, hover, usages and rename
      (`navigate/callable.rs`). Not a reference kind of `frameworks::refs`: what the string
      names is a declaration navigation already knows, so it is resolved there and needs no
      group of always-on kinds. A parameter that also takes a plain string or array
      (`callable|string`) stays a possible usage.
- [x] Function names complete inside such a string (the full name, escaped in a
      double-quoted one).
- [x] Budget runs before and after, on the fixture and on laravel/framework: see below.

Budgets on laravel/framework, before (the phase 2 commit) and after: hover 2.3 and 2.5 ms,
completion 6.7 and 7.0 ms, diagnostics after an edit 209 and 215 ms (budget 300), index
from an empty cache 1353 and 1496 ms (budget 30000). Within what two runs differ by: the
strings are only looked at for a request at a caret, and in the files a usage search
already reads.

Left over from phase 2.5:

- [x] The method completes in `[$o, '…']` (the static ones after `Foo::class`) and after
      `'Foo::` in a single-quoted string.
- [x] A string a function or method declared to return a callable returns, and one
      assigned to a property typed as a callable (`$this->then = [$this, 'run']`), are
      seen like arguments.

Moved to phase 3, where arrays of callbacks are common: a callable an array's value type
expects, a closure's own return type, a default value.

## Phase 3 — WordPress

On branch `wordpress`, uncommitted.

- [x] The generic name facts: `summary/names.rs` (`NameFacts`: what a call declares, by
      `DeclaredKind`), a table of declaring calls in `summary/extract/names.rs`, one index
      key for the files that have some, and `PhpEngine::names()` (`frameworks/names.rs`). A
      new kind of name is an enum variant and a table row. Only declarations are facts:
      usages are found through the word index like those of any string reference, so the
      planned "used" half wasn't needed. Summary schema 39.
- [x] Hooks (`wordpressHook`): fired by `do_action` / `apply_filters` and their
      `_ref_array` and `_deprecated` forms; named in `add_action`, `add_filter`,
      `remove_*`, `has_*`, `did_*`, `doing_*`. Completion, goto, usages, hover, rename
      (the project's own: not WordPress's or a library's). A name put together at run time
      is a pattern (`save_post_*`).
- [x] `PhpUndefinedHook`, off by default: the first opt-in inspection
      (`inspect::OPT_IN`, turned on with `"enabled": true`).
- [x] Callbacks of `add_action`, `add_filter`, `add_shortcode` and the rest: the phase 2.5
      string callables, from the `@param callable` of WordPress's own docblocks.
- [x] Callbacks under a key of an array argument (`register_rest_route(…, ['callback' =>
      …])`, `sanitize_callback`, `render_callback`…): a table in `navigate/callable.rs`
      (`KEYED`), since WordPress documents those arrays in prose, not as shapes.
- [x] Shortcode tags, post types, taxonomies, script and style handles, option names, image
      sizes and nav-menu locations: one `WordPressName` per registry
      (`frameworks/wordpress/refs/names.rs`), with the same features as hooks.
- [x] Template parts (`wordpressTemplatePart`): `get_template_part()` goes to the theme's
      file and completes its templates. No rename.
- [x] Parameter types of a hook's callback, for a closure or arrow function given to
      `add_action` / `add_filter`: the `@param` types of the docblock in front of the
      firing statement (`DeclaredName::params`, `Resolver::hook_parameters`).
- [x] Types of `global $wpdb, $post…`: the function's own `@global` tag, else what the
      project's `@global` tags say of the name (`Resolver::global_type`). In any PHP
      project.
- [x] The `wordpress` stub examples are gone (`base/settings.rs`, `package.json`, the
      schema); the READMEs document the plugin-only setup (`includePaths` or a stubs
      package).
- [x] Tests: `navigate/tests/wordpress.rs` on a stand-in for WordPress's API written for
      the tests, and a server test of a plugin opened on its own.

Left over from phase 3:

- [x] A function or method registered as a hook's callback by name (`'shop_saved'`,
      `[$this, 'saved']`, `[Foo::class, 'saved']`) gets the parameters nothing else types
      from the hooks it is registered for (`NameFacts::callbacks`,
      `Resolver::hook_callback_parameter`).
- [x] A global variable read at the top level of a file without being set there, and
      `$GLOBALS['name']`, have the documented type too.
- [x] WordPress itself: `tests/wordpress_corpus.rs` (ignored, on the `wordpress` corpus
      that `scripts/php-corpus.sh` already had). It found that `init` is fired from
      `wp-settings.php`, beside the core's folders, which the rename rule didn't count as
      WordPress's; fixed. The undefined-member check on the corpus reports the same 2
      problems with and without this phase (1,301 files).
- [x] The VS Code status item names the frameworks of the open projects, Symfony and
      Laravel included: the server's Laravel status request and notification carry
      `frameworks`.
- [x] `docs/engine/wordpress.md`.

- [x] Lists of names: `is_singular(['book', 'page'])`, `'post_type' => ['book']` in a
      query, the post types of a taxonomy and the taxonomies of a post type
      (`Position::ListOfKey` for a list under a key).
- [x] The theme's other files (`wordpressThemeFile`): `get_header('name')`,
      `get_footer()`, `get_sidebar()`, `locate_template()`, `get_theme_file_path()` and
      `get_theme_file_uri()`.
- [x] Blocks registered in PHP by name (`wordpressBlock`, `DeclaredKind::Block`,
      `SCHEMA_VERSION` 40).
- [x] `fixtures/wordpress-app` (a plugin and a theme on a stand-in API written for it)
      and `tests/wordpress_fixture.rs`: nothing in the plugin or the theme is reported,
      and one name of each kind resolves.

- [x] Blocks a `block.json` declares (`frameworks/wordpress/assets.rs`,
      `PhpEngine::wordpress_blocks`; the server watches `**/block.json`).
- [x] REST routes (`wordpressRestRoute`, `DeclaredKind::RestRoute`, `SCHEMA_VERSION` 41):
      a namespace and a route declared, one path named; patterns as `*`.
- [x] Scripts in Find References and Rename, searched as text: a block's name, a REST
      route's path, the action of an AJAX request for its `wp_ajax_…` hook.
- [x] A child theme's parent, from the `Template:` header of its `style.css`.
- [x] A string under a key named `callback`, `…_callback` or `…_cb` is a callable in any
      call (`navigate/callable.rs`), not only the listed ones.

- [x] REST routes whose namespace and base are properties of the controller
      (WordPress's own), and rename of a route's last part (`SCHEMA_VERSION` 42).
- [x] Rename of an AJAX action: both hooks and the scripts
      (`ReferenceKind::site_rename_range`, `other_renames`).
- [x] `register_block_type('build/price')` is a folder when there is one.
- [x] Scripts and `block.json` as documents (`frameworks/wordpress/scripts.rs`, the
      server's `Kind::WordPressScript`): goto, hover,
      Find References, Rename and completion of blocks, REST routes and AJAX actions.
- [x] Scripts are sent to the server only in a WordPress project: the server registers
      for them (`Server::register_scripts`), the extension's selector doesn't list them.
      The end-to-end suite runs once on `fixtures/wordpress-app`: 12 checks in VS Code,
      one per feature of this phase.

What is left is at the end of `wordpress.md`: a script is read as text, not parsed.

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
