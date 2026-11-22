# Framework support

How the engine knows which frameworks a project uses, and where a framework plugs in.
Laravel's own notes are in [laravel.md](laravel.md).

## What exists

| Framework | Detection | Types | String references |
| --- | --- | --- | --- |
| Laravel | yes | `types::provider::LaravelProvider` | 24 kinds (`frameworks::laravel::refs`) |
| Symfony, Doctrine | yes | `types::provider::SymfonyProvider` | route names, template names, service ids, parameters, translation keys and Doctrine fields (`frameworks::symfony::refs`) |
| WordPress | yes | none (hook callbacks and `global` variables are typed by the engine itself, from name facts) | hooks, shortcodes, post types, taxonomies, script and style handles, options, image sizes, menu locations, template parts (`frameworks::wordpress::refs`; see [`wordpress.md`](wordpress.md)) |
| Drupal | yes | `types::provider::DrupalProvider` (services, entity storages); hook implementations by the engine itself | services, routes, permissions, libraries, configuration, modules, entity types, plugins, theme hooks, hooks (`frameworks::drupal::refs`; see [`drupal.md`](drupal.md)) |
| CodeIgniter (4, and 3) | yes | `types::provider::CodeIgniterProvider` (helper functions, a model's finders, a builder's table, what CodeIgniter 3's loader gives a controller); a view's variables by the engine itself | views, helpers, language lines, routes, filters, route handlers, configurations, models, services, view cells, events, validation rules and groups, tables and columns; 3's libraries, configuration items and controller paths (`frameworks::codeigniter::refs`; see [`codeigniter.md`](codeigniter.md)) |
| Yii 2 | yes | `types::provider::YiiProvider` (components, the identity, `createObject()`, properties read through a getter, Active Record queries and relations, a record's columns from the migrations); a view's variables by the engine itself | views, layouts, mail views, routes, components, modules, parameters, aliases, message categories and messages, model attributes (`frameworks::yii::refs`; see [`yii.md`](yii.md)) |
| CakePHP 4 and 5 | yes | `types::provider::CakePhpProvider` (tables, a table's entities and queries, association properties, a controller's components, a view's helpers, behaviours); a template's variables by the engine itself | tables, components, helpers, behaviours, associations, templates, layouts, elements, cells, configuration keys, controllers, actions, plugins, named routes (`frameworks::cakephp::refs`; see [`cakephp.md`](cakephp.md)) |
| Laminas, Magento | yes | none yet | none yet |

Twig templates are not a framework in this sense: `phoxite::twig` reads a template's text
and the project's template folders, needs no engine, and is served for every `*.twig` file.
Whether Symfony's Twig extensions are known there depends on Composer installing its bridge
(`twig::TwigProject`).

## Detection

`frameworks::detect::Frameworks::detect(root, settings)` answers with a set of
`Framework`s. A framework is in it when Composer installs one of its packages
(`composer.json`, `composer.lock`, `vendor/composer/installed.json`, read through
`ComposerPackages`) or when one of its own files exists:

| Framework | Composer package | Files |
| --- | --- | --- |
| Laravel | `laravel/framework` | `artisan` |
| Symfony | `symfony/framework-bundle` | (the `FrameworkBundle` class being known, see below) |
| WordPress | `johnpbloch/wordpress`, `johnpbloch/wordpress-core`, `roots/wordpress`, `roots/wordpress-no-content`, `php-stubs/wordpress-stubs` | `wp-config.php`, `wp-load.php`, `wp-includes/version.php`; a root PHP file with a `Plugin Name:` header; a `style.css` with a `Theme Name:` header |
| Drupal | `drupal/core`, `drupal/core-recommended` | `core/lib/Drupal.php` (also below `web/` and `docroot/`); a root `*.info.yml` with a core requirement |
| CodeIgniter | `codeigniter4/framework`, `codeigniter/framework` | `spark`, `system/core/CodeIgniter.php` |
| Yii | `yiisoft/yii2` | `yii`, `vendor/yiisoft/yii2/Yii.php` |
| CakePHP | `cakephp/cakephp` | `bin/cake`, `vendor/cakephp/cakephp/VERSION.txt` |
| CakePHP | `cakephp/cakephp` | |
| Laminas | `laminas/laminas-mvc`, `mezzio/mezzio` | |
| Magento | `magento/framework`, `magento/product-community-edition`, `magento/product-enterprise-edition` | `app/etc/di.xml`; `registration.php` with `etc/module.xml` |

The `frameworks` setting forces each one `enabled` or `disabled` (`auto` by default).
`frameworks.laravel` wins over the older `laravel.detection` unless it is `auto`.

`PhpEngine::frameworks()` is what the rest of the engine asks. It keeps the answer until a
watcher batch touches a file detection reads. Two frameworks are settled outside
`Frameworks::detect`:

- Laravel is in the set exactly when `PhpEngine::laravel()` has a project model, so the two
  never disagree.
- Symfony is also in the set when its `FrameworkBundle` class is known to the index though
  Composer doesn't install the bundle (Symfony's own source tree). That is asked once per
  modification count.

## String references

`frameworks::refs` is shared by every framework: place patterns (`PlacePattern`,
`CalleePattern`), the string sites of a file (`sites_with`), the kind trait
(`ReferenceKind`) and the engine's requests (`string_ref_completion`, `_hover`,
`_definition`, `_references`, `_rename`, `string_highlights`, `undefined_references`).

Each framework lists its kinds, and `refs::kinds_for(frameworks)` gives the kinds of the
frameworks a project uses. A kind is only asked in a project of its framework, so two
frameworks can give the same call (`view('…')`) a meaning each.

`RefContext` carries the engine, the project's frameworks and, in a Laravel project, the
Laravel project model. `RefContext::project()` is for Laravel's kinds and panics elsewhere;
another framework's kind reads its own model from `cx.engine`.

Three things in `frameworks::refs` are still Laravel's: the completion rules of `ide.json`
(read in a Laravel project only), the helper instances `sites` knows (`config()` without
arguments), and the handling of a few Laravel kinds in `undefined_references`.

## Adding a framework

1. **Detection.** A `Framework` variant in `frameworks/detect.rs` with its packages and
   marker files, a field in `base::settings::FrameworkSettings`, and the setting in
   `editors/vscode/package.json` and `editors/vscode/schemas/phoxite.schema.json` (a test in
   `phoxite-lsp/src/config.rs` compares the schema with the settings).
2. **Types.** A provider in `types/provider/<name>.rs` implementing `TypeProvider`,
   registered in `TypeProviders::default()`.
3. **String references.** Kinds in `frameworks/<name>/refs/`, listed by a `kinds()` function
   that `refs::framework_kinds` returns for the framework. A kind with an "undefined …"
   inspection of its own adds its list to `INSPECTIONS` in `inspect/laravel.rs`; route names
   are reported by `inspect/routes.rs` (`PhpUndefinedRoute`).
4. **Facts from PHP files.** For a name a call declares (a hook, a post type): a
   `DeclaredKind` in `summary/names.rs` and a row of the table in
   `summary/extract/names.rs`. `PhpEngine::names()` then lists every declaration with its
   file (`frameworks::names`), and a reference kind reads it from there; nothing else
   needs plumbing. For other facts: a struct in `summary/`, a field of `FileSummary`, the
   walker hooks in `summary/extract.rs` and an index key in `index/tables.rs`. Either way,
   a bump of `SCHEMA_VERSION` (`summary/mod.rs`).
5. **Configuration files.** `base::yaml` reads the YAML that Symfony's and Drupal's files
   use. `frameworks::php_array` reads a PHP file that returns an array (Yii's
   configuration, Symfony's `services.php`): `return $config;`, `require`, `array_merge()`,
   `Foo::class` keys and class-constant values; see [`yii.md`](yii.md).
6. **Design inputs.** The framework's public documentation, and its source where the
   licence allows it (see `CONTRIBUTING.md`). WordPress and Drupal are GPL and Magento is
   OSL: read their documentation for behaviour and copy nothing from them. Names (hooks,
   services, events) come from indexing the user's project, never from a list in this
   repository.
