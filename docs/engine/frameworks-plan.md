# Framework support: plan and progress

Tracks the plan to support what Phoxite doesn't cover yet: Twig in the server and VS Code,
deeper Symfony, and WordPress, Drupal, CodeIgniter, Yii, CakePHP, Laminas, Magento and
Joomla.
How the pieces fit is in [frameworks.md](frameworks.md).

Depth for each new framework: types, plus completion, goto, usages, rename and an
undefined-name inspection for the strings that name things. No tool views in VS Code.

Last updated: 2026-11-22.

## Status

| Phase | What | Status |
| --- | --- | --- |
| 0 | Framework-neutral foundations | Done |
| 1 | Twig, end to end | Done |
| 2 | Symfony and Doctrine, deeper | Done |
| 2.5 | Callable-string references | Done |
| 3 | WordPress | Done |
| 4 | Drupal | Done |
| 5 | CodeIgniter (4, and 3) | Done |
| 6 | Yii 2 | Done |
| 7 | CakePHP 4 and 5 | Not started |
| 8 | Laminas (MVC and Mezzio) | Not started |
| 9 | Magento 2 | Not started |
| 10 | Joomla 6 | Not started |

Order: 0, then 1 → 2 → 4. Phase 2.5 comes before phase 3. Phases 3 and 5–8 need only phase
0 and can be reordered. Phases 9 and 10 come last: they share the XML reader, which is
built with whichever of them comes first.

## Decisions

- [x] The PHP-array reader is `frameworks/php_array.rs`, not `base/`: it resolves
      `Foo::class` through the file's namespace and imports, so it needs the parser and
      the name scopes, which `base` is below.

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
- **0.5b PHP-array reader:** moved to phase 6 (done there: `frameworks/php_array.rs`).
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
      …])`, `sanitize_callback`, `render_callback`…): by the key's name
      (`navigate/callable.rs`), since WordPress documents those arrays in prose, not as
      shapes. (Since phase 4 in any array, not only a call's argument.)
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

Needs phases 1 and 2. See `drupal.md`.

- [x] `.module`, `.install`, `.theme`, `.profile`, `.inc` and `.engine` are PHP
      (`project::PHP_EXTENSIONS`, in every project; the server watches them; the VS Code
      extension associates them with PHP).
- [x] The YAML files (`frameworks/drupal/project.rs`, `PhpEngine::drupal`): services,
      routes, permissions, libraries, extensions, configuration names.
- [x] Service ids: names, and the types of `\Drupal::service('id')` and
      `$container->get('id')` (`types/provider/drupal.rs`; the services go into the
      configuration the Symfony provider answers from).
- [x] Route names: `Url::fromRoute()`, `Link::createFromRoute()`, `redirect()`,
      `setRedirect()`, and `route_name:` of the `*.links.*.yml` files.
- [x] Hooks: the `hook_…()` functions of `*.api.php`, `invokeAll()` / `alter()` calls,
      `#[Hook]` methods, the functions implementing one as its usages, and the parameter
      types of an implementation.
- [x] Plugin ids from attributes and annotations to `createInstance('id')`.
- [x] Entity type ids, with the storage and the entity classes of `getStorage('node')`.
- [x] Permissions, library names, config names, module names, theme hooks.
- [x] Strings under a key of an array as a place (`CalleePattern::Entry`): `#theme`,
      `#attached`.
- [x] Template folders of modules and themes in `TemplatePaths` (`@shop/…`).
- [x] `fixtures/drupal-app`, `tests/drupal_fixture.rs`, the server's test, a Drupal run of
      the VS Code end-to-end suite (10 checks), and `tests/drupal_corpus.rs` on Drupal 11
      (`scripts/php-corpus.sh drupal`). The checker on four core modules (1,886 files)
      reports nothing that comes from this phase.

- [x] The YAML files as documents (`frameworks/drupal/documents.rs`, the server's
      `Kind::DrupalYaml`, registered for only in a Drupal project): goto, hover, usages,
      rename and completion of the names they write, and goto on their classes.
- [x] Configuration keys (`drupalConfigKey`), plugins of links and layouts files,
      derivative plugins, theme hook suggestions and preprocess functions.
- [x] Go to Definition from a hook's implementation to the hook; a template's variables
      and `attach_library()`; no unused-parameter reports in `*.api.php`.
      `SCHEMA_VERSION` 44.

- [x] The key above a YAML line, `service:method`, class completion in YAML; the config
      schema; migrations and breakpoints; `#[Hook]` on a class; the types of a template's
      variables from its preprocess functions (`frameworks/drupal/templates.rs`).
- [x] Drupal 7: detection, and its functions for hooks, theme hooks, permissions and
      modules. `SCHEMA_VERSION` 45.

- [x] Drupal 7's menu paths (`hook_menu()`), variables, form functions
      (`drupal_get_form()`), menu callbacks and `.info` files as documents;
      `tests/drupal_corpus.rs` runs on Drupal 7 too (`scripts/php-corpus.sh drupal7`).
- [x] A config schema types `->get('key')`; a render array types its template's
      variables; brace mappings and aliases in YAML. `SCHEMA_VERSION` 46.
- [x] A callback under a key named as one is a callable in any array (it was only in a
      call's argument).

- [x] Anchors, aliases and `<<` merges in the YAML reader (`base/yaml.rs`); database
      tables (`drupalTable`, from `hook_schema()`); config key types from installed
      values; render arrays built up in a function. `SCHEMA_VERSION` 47.

- [x] A table's columns (`drupalColumn`, `refs/column.rs`; queries are typed by their
      table); a render array returned by a function of the file. `SCHEMA_VERSION` 48.

- [x] A query keeps its table along a chain; a column after the alias of a joined table;
      a render array built by a function of another file.

Nothing is left that the project's files say: the end of `drupal.md` lists what can't be
read (Drupal 7's untyped hook documentation, values that cross calls).

## Phase 5 — CodeIgniter (4, and 3)

See `codeigniter.md`.

- [x] The helper-instance table per framework: left as it is. Its classes are only in the
      patterns of Laravel's kinds, which are asked in a Laravel project only; gating it on
      the class being known broke Laravel projects whose stubs don't declare it.
- [x] Types: `service('name')`, `single_service()`, `model()`, `config()`
      (`types/provider/codeigniter.rs`); `Services::name()` and the functions a
      `helper()` call loads need nothing (they are declarations the index has). A
      model's finders by its `$returnType`; `$db->table('books')` as a builder of that
      table; the closure of a route group.
- [x] Names: views, helpers, language lines, routes (by `as`, path or handler), filter
      aliases, configurations, models, services, a route's handler, view cells, events,
      validation rules and rule groups, tables and columns
      (`frameworks/codeigniter/refs`). `SCHEMA_VERSION` 50 (events, tables and columns
      are name facts).
- [x] A view's variables from the `view()` calls passing them
      (`Declarations::view_variable`), shared with its layout and its partials; `$this`
      in a view (`TEMPLATE_THIS`).
- [x] An inline `@var` declares a variable nothing sets (`$routes`).
- [x] Modules: the `$psr4` of `Config/Autoload.php` and Composer's namespaces; a moved
      application, framework or views folder by `Config/Paths.php`.
- [x] Fifteen "unknown name" inspections (`inspect/laravel.rs`, `CODEIGNITER`; the
      event, table and column ones off unless turned on), checked on CodeIgniter's own
      application, framework, tests and user guide examples. `SCHEMA_VERSION` 51.
- [x] Several columns in one string, a column in an expression, the table and field of
      `is_unique[…]`; the routes `resource()` and `presenter()` make; a path as its route
      or the method auto-routing would call; `dirname()` and `realpath()` in the paths
      configuration. `SCHEMA_VERSION` 52.
- [x] CodeIgniter 3 (left out of the plan "unless asked for"; asked for): the loader's
      names, language lines, configuration items, controller paths (through
      `config/routes.php`), what the loader gives a controller (a model under another
      name and a library of the application among it), a view's variables, the tables
      and fields of its migrations.
- [x] `fixtures/codeigniter-app`, `tests/codeigniter_fixture.rs`, the server's test, a
      CodeIgniter run of the VS Code end-to-end suite, and `tests/codeigniter_corpus.rs`
      on CodeIgniter 4.6 and 3.1.13 (`scripts/php-corpus.sh codeigniter codeigniter3`).

What is left is at the end of `codeigniter.md`: a builder whose table isn't a string, the
fields other rules name, `only`/`except` of a resource, third-party HMVC.

## Phase 6 — Yii 2

On branch `yii`, uncommitted. See `yii.md`.

- [x] First: the PHP-array reader (`frameworks/php_array.rs`). A reusable reader for PHP
      files that return an array, with `Foo::class` keys and class-constant values
      (Laminas needs those). It also follows what Yii's files are written with:
      `return $config;` and the `$config['a']['b'] = …` after it, a variable for a value,
      `require` of a path written with `__DIR__` and `dirname()`, `array_merge()` and
      `ArrayHelper::merge()`. Laravel's `config.rs` uses its helpers; its own walk stays
      (it keeps each value's source text, which the reader doesn't).
- [x] Then, for Symfony (moved from phase 2, where only the reader was missing):
  - [x] `config/services.php`, `config/routes.php` and the PHP files of
        `config/packages` and `config/routes` where they return plain arrays
        (`return [...]`, `return App::config([...])`): their services, parameters and
        routes beside the YAML files' (`symfony/mod.rs`, which reads the array as the
        YAML reader's mapping). A file returning a closure says nothing.
  - [x] PHP translation catalogues (`translations/messages.en.php`): their keys beside
        the YAML and XLIFF ones (`symfony/translations.rs`).
- [x] The project model (`frameworks/yii/project.rs`, `PhpEngine::yii()`): the
      applications of the basic and the advanced template, components, modules,
      parameters, aliases, controller namespaces, the identity class, messages.
      Detection also by the `yii` script and `vendor/yiisoft/yii2/Yii.php`.
- [x] Types (`types/provider/yii.rs`): `Yii::$app->…` components from the config arrays
      (and `get('id')`, and the identity of `user`), `Yii::createObject()`,
      `Yii::$container->get()`, `ActiveQuery<Model>` with `one()` and `all()`, relation
      properties, and any property read through a getter. Components and getters are
      members for completion, hover and goto (`AttributeOrigin::Configured`, `Getter`).
- [x] Names (`frameworks/yii/refs`): views, layouts, mail views, routes
      (`Url::to(['site/index'])`), components, modules, path aliases, translation
      categories and messages, attribute names in `rules()`, `attributeLabels()`,
      `attributeHints()` and `scenarios()`.
- [x] A view's variables from the `render()` passing them, and `$this` in a view
      (`frameworks/yii/views.rs`).
- [x] Four "unknown name" inspections: `PhpUndefinedYiiView`, `PhpUndefinedYiiLayout`,
      `PhpUndefinedYiiRoute`, `PhpUnknownYiiAttribute`.
- [x] Engine tests (`navigate/tests/yii.rs`) and `docs/engine/yii.md`.

- [x] `fixtures/yii-app` (an application laid out like the basic template, on a stand-in
      for Yii in `lib/`), `tests/yii_fixture.rs` (nothing in the application is
      reported, one name of each kind resolves, what is passed and configured is typed)
      and the server's test (`a_yii_application_names_its_views_and_types_them`; the
      status names the framework).
- [x] The corpus: `scripts/php-corpus.sh yii yii-framework` (Yii's basic application
      template and the framework, both 2.0.55; the framework is an include path, as the
      template has no `vendor/` until Composer runs) and `tests/yii_corpus.rs`. On the
      real source the configuration is read (16 components over its three files, the
      identity class, the parameters), views and routes resolve, `Yii::$app->cache`,
      `->db` and `->user->identity` are typed, and the four Yii inspections report
      nothing in the template's 20-odd files. What the other inspections say there is
      the classes of the extensions the corpus doesn't have, and one true report: the
      template's `models/LoginForm.php` at 2.0.55 has a docblock that is never closed
      before its class.
- [x] `SCHEMA_VERSION` 54 (52 on `main`): a summary keeps the question of `Post::find()`, `hasOne()`,
      `hasMany()` and `createObject()` for the Yii provider.

- [x] The VS Code side: a Yii run of the end-to-end suite (`yiiChecks`, 5 checks on the
      fixture, pass in VS Code 1.147.0; `PHOXITE_E2E_ONLY=yii`), the settings' text and
      the schema. The status names the framework without client code: the server sends
      every detected one.
- [x] README, `editors/vscode/README.md` and both changelogs (under "Unreleased").
      `docs/editors.md` needed nothing: no new request or command.
- [x] Budget runs on laravel/framework, `main` and then the branch: hover 2.30 and 2.23
      ms, completion 6.91 and 6.78 ms, diagnostics after an edit 210 and 209 ms (budget
      300), index from an empty cache 1344 and 1373 ms (budget 30000), warm start 55 and
      54 ms. Within what two runs differ by. The provider keeps a question only for
      `find()`, `hasOne()`, `hasMany()` and `createObject()`; `get()`, `one()` and
      `all()` are answered when an expression is typed, not kept in a summary.
- [x] The advanced application template in the corpus (`yii-advanced`, 2.0.55): four
      applications, each with its own views, controllers and parameters; the aliases of
      its bootstrap file; the identity class; no Yii inspection in its 62 files.
- [x] Parameters as names (`yiiParam`: `Yii::$app->params['adminEmail']`, and one below
      another, `params['mail']['from']`): the string references read the keys of an
      array access on a property of a class asked about (`CalleePattern::Read`, with the
      keys read before in `CallTarget::Read`). A view's own `$this->params` isn't the
      application's.
- [x] A name in a view whose receiver is a variable the view is passed
      (`$form->getAttributeLabel('email')`): `refs::sites_at` types a file's receivers
      knowing its path. For every framework's views.
- [x] A component, a module or a parameter several applications configure goes to the
      asking application's own.
- [x] Usages of a Symfony service or parameter inside a PHP configuration file
      (`'@mailer'`, `'alias' => 'mailer'`, `'%app.locale%'`).

- [x] A record's columns from the project's migrations (`frameworks/yii/migrations.rs`,
      `PhpEngine::yii_tables()`): typed, completed, a target for goto, and attributes of
      `yiiAttribute`. A migration is read again only when its file changes.
- [x] `controllerMap`, the actions `actions()` lists, the routes of the URL manager's
      rules and of a filter's `only`, `except` and `actions`. Where several applications
      have the controller, a route is the one that has the action.
- [x] Views where the configuration puts them: `viewPath`, `layoutPath`, the mailer's
      `viewPath`, themes (`YiiProject::actual` and `logical`).
- [x] Gettext catalogues (`.po`), the aliases of `vendor/yiisoft/extensions.php` and of
      `Yii::setAlias()` in every file of `config/` and the entry scripts,
      `Yii::createObject(['class' => …])`.
- [x] Budgets again after these, on laravel/framework: hover 2.35 ms, completion 6.84 ms,
      diagnostics after an edit 215 ms, index from an empty cache 1454 ms, warm start 55
      ms. As before, within what two runs differ by.
- [x] The end-to-end runner takes a relative `PHOXITE_SERVER` (it was read from the
      project VS Code opened, where it names nothing).

Nothing of the phase is open. Not run by hand in VS Code: the phase adds no client code
and nothing to look at, and the end-to-end suite asks what the editor asks.

What cannot be read without running the application (a table no migration makes,
messages in a database, names built at run time) is at the end of `yii.md`.

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

## Phase 10 — Joomla 6

Needs the XML reader (phase 9; build it here if this phase comes first).

The target is Joomla 6: 6.2.0 (13 October 2026) is the current release, on PHP 8.3 or
later (looked up 2026-11-22; sources at the end of this phase). The fixture and the corpus
are Joomla 6.2. Joomla 5.4 and 4 have the same namespaced API and extension layout, so
most of this should work there, but nothing is checked on them. Joomla 3's unnamespaced
classes (`JFactory`, `JText`) are left out unless asked for.

What Joomla 6 changed that this phase follows:

- `Table::getInstance('content')` is replaced by the class itself
  (`new \Joomla\CMS\Table\Content($db)`), so a table is a plain `new`; the names to
  resolve are those of the MVC factory (`createTable()`, `getTable()`).
- `\Joomla\CMS\Input`, `\Joomla\CMS\Filesystem`, `BaseApplication` and `CliApplication`
  are gone from the core (`\Joomla\Input`, `\Joomla\Filesystem`, `\Joomla\Application`
  instead) and live on in the "Behaviour - Backward Compatibility 6" plugin, which an
  upgraded site has enabled. Nothing to build: where the plugin's files are in the
  project they are declarations the index has; where they aren't, the classes are
  unknown, as they should be.
- A plugin's `$app` property is gone: `$this->getApplication()` is what gets a type.
- `AdminModel::getItem()` returns a `stdClass`, not a `CMSObject` (out of the core in
  6.0, removed in 7.0): an item's fields can't be typed from the class, only from the
  table's columns, where the table is known.
- Events are classes: `onContentPrepare` is
  `\Joomla\CMS\Event\Content\ContentPrepareEvent`, `onContentBeforeSave` is
  `\Joomla\CMS\Event\Model\BeforeSaveEvent`. The manual lists the class per event and
  gives no rule, so the map has to be read from the core (believed to be
  `Joomla\CMS\Event\CoreEventAware`; check it in the corpus first).

- [ ] Detection: no Composer package names a Joomla site or an extension opened on its
      own, so by the files too (`libraries/src/Version.php`, an extension's manifest
      XML).
- [ ] An INI reader for the language files (`language/en-GB/*.ini`).
- [ ] Types: `Factory::getApplication()` and what it hands out (`getIdentity()`,
      `getInput()`, `getDocument()`); `bootComponent('com_content')` and
      `getMVCFactory()`; `createModel('Article')`, `createTable()` and a controller's
      `getModel()` by the component's namespace; `$container->get()` from the
      `services/provider.php` files; `$this` in a view's `tmpl/*.php` and in a layout;
      `$this->getApplication()` in a plugin; the parameter of a plugin's handler, where
      nothing types it, from the event class of the name it subscribes to.
- [ ] Classes of an extension opened on its own: the `<namespace path="src">` of its
      manifest is a PSR-4 root (no `composer.json` names it), like CodeIgniter's
      `$psr4`.
- [ ] Names: language constants (`Text::_()`, `Text::sprintf()`, `Text::plural()`, and
      the labels and descriptions in XML), layout ids
      (`LayoutHelper::render('joomla.content.tags')`), view templates and their
      overrides in a site template's `html/` folder, `HTMLHelper::_('behavior.core')`
      keys, web asset names (`useScript()`, `useStyle()`, `usePreset()` from
      `joomla.asset.json`), component, module and plugin names, plugin events
      (dispatched, and the keys of `getSubscribedEvents()`, with goto to the event's
      class and to the handler method its value names), ACL actions
      (`authorise('core.edit')` from `access.xml`), form names and their fields
      (`loadForm()`, `$form->getValue()` from the XML forms), parameters
      (`$params->get()` from `config.xml` and the manifests), tables written with the
      `#__` prefix and their columns, and `option`, `view` and `task` in a route string
      (`Route::_('index.php?option=com_content&view=article')`).
- [ ] The XML files as documents (manifests, forms, `access.xml`, `config.xml`), sent to
      the server only in a Joomla project like Drupal's YAML: goto, hover, usages and
      completion of the classes, language constants and field types they write.
      `joomla.asset.json` too, for the asset names and their dependencies.

Not planned: the JavaScript side that 6.1 and 6.2 added (the `media_source` builders,
`Joomla.editorFilePickers`), and warnings for the classes Joomla 6 removed (they are
unknown classes already, unless the compatibility plugin declares them).

Sources: the roadmap (developer.joomla.org/roadmap.html) and the manual's upgrade notes
(manual.joomla.org/updates/54-60, 60-61 and 61-62) and plugin events pages. The 6.1 and
6.2 notes list no deprecations and no PHP API changes that touch this phase.

## Every framework phase (2–10) also delivers

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
