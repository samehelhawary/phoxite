# Settings

Phoxite's settings for one project, at its root. They are laid over the editor's settings, so everyone working on the project gets them whatever their editor. A project can't set the programs Phoxite runs or the files it reads outside the project: `analyzers` and `laravel.database` stay with the editor's settings, and `includePaths` is read only in a workspace you trust. The file is `phoxite.json`.

In VS Code the same settings are under `phoxite.` (`phoxite.format.lineWidth`); another
editor sends them as `docs/editors.md` says. This page is written from the file's schema
(`node scripts/settings_doc.mjs`): don't edit the table by hand. The ids `inspections` takes
are in [inspections.md](inspections.md).

```json
{
  "$schema": "https://raw.githubusercontent.com/samehelhawary/phoxite/main/editors/vscode/schemas/phoxite.schema.json",
  "phpVersion": "8.3",
  "frameworks": { "symfony": "disabled" }
}
```

| Setting | Type | Default | What |
| --- | --- | --- | --- |
| `phpVersion` | string | `""` | The PHP version the code is checked against, e.g. `8.3`. Empty: from `composer.json` (`config.platform.php`, else the lowest version `require.php` allows), else the latest. |
| `includePaths` | array of string | `[]` | Folders and single files outside the project whose code is indexed too: absolute, `~/…`, or relative to the project (`../shared`, `../../config.php`). A folder is indexed whole, so name the file when one file is all you need. A `composer.json` at the top of such a folder is read for what it autoloads, and so is the one beside a Composer `vendor` folder. Read only in a workspace you trust. |
| `stubs` | array of string | `[]` | Extra phpstorm-stubs sets (`redis`, `swoole`, `mongodb`…) on top of the defaults and the `ext-*` requirements of `composer.json`. |
| `exclude` | array or null | `null` | Globs of files and folders left out of the index (`node_modules`, `/storage`). `null`: `node_modules`, `.git`, `storage/framework` and `bootstrap/cache`. `vendor` is read through Composer's `installed.json` either way. |
| `inspections` | object | `{}` | Per-inspection settings by id, e.g. `{ "PhpUndefinedMethod": { "enabled": false } }`. The key `*` with `enabled: false` turns every inspection off. |
| `laravel.detection` | `auto`, `enabled`, `disabled` | `"auto"` | Laravel support: `auto` when `composer.json` requires `laravel/framework`, the lock file lists it, or `artisan` exists. |
| `laravel.routeHints` | boolean | `true` | Code lenses on controller methods listing their routes, and the full URI after each route registration. |
| `laravel.modules` | `auto`, `nwidart`, `directory`, `none` | `"auto"` | Which modules of a modular application are read: `nwidart/laravel-modules`, module folders (`modules/`, `app-modules/`…), both (`auto`), or none. |
| `laravel.inertiaPages` | array of string | `[]` | Folders of Inertia pages, relative to the project. Empty: the `ide.json` page paths, else `resources/js/Pages`. |
| `laravel.generation` |  |  | What the Laravel generators write. |
| `laravel.generation.namespace` | string | `""` | The application's root namespace for generated classes. Empty: the one `composer.json` maps to `app/`. |
| `laravel.generation.module` | string | `""` | The module generated classes go to. Empty: the application. |
| `laravel.generation.controllerSuffix` | string | `"Controller"` | The suffix of generated controller classes. |
| `laravel.generation.requestSuffix` | string | `"Request"` | The suffix of generated request classes. |
| `laravel.generation.resourceSuffix` | string | `"Resource"` | The suffix of generated resource classes. |
| `laravel.generation.policySuffix` | string | `"Policy"` | The suffix of generated policy classes. |
| `laravel.generation.seederSuffix` | string | `"Seeder"` | The suffix of generated seeder classes. |
| `laravel.generation.constructorPromotion` | boolean | `true` | Generated constructors promote their parameters to properties. |
| `laravel.generation.typeHints` | boolean | `true` | Generated methods declare their return types. |
| `frameworks` |  |  | Whether each framework's support is on: `auto` (when Composer installs the framework or one of its own files exists), `enabled` or `disabled`. `laravel` wins over `laravel.detection` unless it is `auto`. Laravel, Symfony, WordPress, Drupal, CodeIgniter, Yii, CakePHP, Laminas, Magento and Joomla have support. |
| `frameworks.laravel` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.symfony` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.wordpress` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.drupal` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.codeigniter` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.yii` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.cakephp` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.laminas` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.magento` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `frameworks.joomla` | `auto`, `enabled`, `disabled` | `"auto"` |  |
| `format.lineWidth` | integer | `120` | The line length Format Document tries to stay within. |
| `format.tool` | `phoxite`, `pint` | `"phoxite"` | What formats a whole PHP file: `phoxite` (the built-in formatter) or `pint` (the project's Laravel Pint). A selection, and Blade files, are always formatted by Phoxite. |
| `format.twig` | `phoxite`, `prettier` | `"phoxite"` | What formats a whole Twig template: `phoxite` (the built-in formatter: indentation and the spacing inside tags) or `prettier` (the project's Prettier with a Twig plugin, which also rewraps lines and formats the JavaScript and CSS inside). A selection is always formatted by Phoxite. |
| `inlayHints.arrayShapes` | `compact`, `full` | `"compact"` | How an array shape reads in a type inlay hint: `compact` (`array`, once in a union of shapes) or `full` (with its keys and their types, `array{id: int, name: string}`). The hint's tooltip and hover show the full type either way. |
| `templates.path` | string | `""` | The folder of the edited generator templates, relative to the project. Empty: `.phoxite/templates`. |
