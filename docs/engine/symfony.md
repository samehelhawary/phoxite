# Symfony and Doctrine

What the engine knows about a Symfony project: the types of its container and of Doctrine's
repositories, the names its strings take (routes, templates, services, parameters,
translation keys, the fields of an entity), and what PHP knows about a Twig template. It
follows Symfony's and Doctrine's public documentation and their MIT source.

Where it lives:

| Part | Module |
| --- | --- |
| What the configuration says: services, parameters, routes | `crates/phoxite/src/symfony` (`SymfonyConfig`, `SymfonyProject`) |
| The translation catalogues | `crates/phoxite/src/symfony/translations.rs` |
| The names in strings | `crates/phoxite/src/frameworks/symfony/refs` |
| What PHP knows about a template | `crates/phoxite/src/frameworks/symfony/templates.rs` |
| The types | `crates/phoxite/src/types/provider/symfony.rs` (`SymfonyProvider`) |
| A route name that names nothing | `crates/phoxite/src/inspect/routes.rs` |

## Detection and configuration

A project is a Symfony one when Composer installs `symfony/framework-bundle`, or when its
`FrameworkBundle` class is known to the index though Composer doesn't install the bundle
(Symfony's own source tree, a bundle under development). The `frameworks.symfony` setting
forces it on or off.

`SymfonyConfig::load` reads the configuration from disk, and the engine keeps it until a
watcher batch touches a YAML file:

- **Services** from `config/services.yaml`, `config/services_*.yaml`, the files of
  `config/packages` and the files their `imports` name (relative paths, folders and `*`
  globs; `when@env` sections too): `id: { class: … }`, the aliases `id: '@other'` and
  `id: { alias: other }`, `parent:` definitions, and `App\Foo: ~` class ids. Namespaces
  loaded from a resource (`App\: { resource: … }`) aren't listed: their classes are their
  ids, which the type provider knows without the configuration.
- **Parameters** from the `parameters:` of the same files: their names, and their values
  when they are scalars.
- **Routes** from `config/routes.yaml`, `config/routes_*.yaml` and the YAML files below
  `config/routes/`: every top-level key with a `path` (or an `alias`), `when@env` sections
  included. An entry with a `resource` imports other routes and is not one itself. The
  `#[Route]` attributes are recorded by the file summaries (`summary::SymfonyFacts`), a
  class-level attribute's name prefixing its methods' names.

The YAML is read with the small reader in `base::yaml`. A PHP file in the place of a YAML
one (`config/services.php`, `config/routes.php`, a file of `config/packages`) is read the
same way where it returns an array of that shape (`return ['services' => [...]]`,
`return App::config([...])`), through `frameworks::php_array`.

**Translations** are the files of the `translations/` folder, named as Symfony names them:
`messages.en.yaml` is the domain `messages` in the locale `en`, and
`messages+intl-icu.en.yaml` is the same domain. In YAML, JSON and PHP catalogues a key is a
top-level key, or the keys of nested mappings joined by dots (`form: { title: … }` is
`form.title`). In XLIFF (`.xlf`, `.xliff`; versions 1.2 and 2.0) a key is a unit's `resname`
(or `name`), else the text of its `<source>`, and the translation is its `<target>`.

## Names

The kinds are in `frameworks/symfony/refs`, on the machinery every framework shares
(`frameworks::refs`): each completes, goes to its declaration, has usages and, where it
says so below, is renamed.

| Kind | Where a string is one | Declared by |
| --- | --- | --- |
| `symfonyRoute` | the first argument of `generateUrl()` and `redirectToRoute()` on a controller, of `generate()` on a URL generator, and of Twig's `path()` and `url()` called from PHP | the route's key in a YAML routes file, or the `name` of its `#[Route]` attribute |
| `symfonyTemplate` | the first argument of `render()`, `renderView()`, `renderBlock()`, `renderBlockView()`, `renderForm()` and `stream()` on a controller, of `render()`, `display()`, `load()` and `resolveTemplate()` on Twig's environment, and of the `#[Template]` attribute | the template's file |
| `symfonyService` | the first argument of `get()` and `has()` on a PSR or Symfony container | its key in a services file |
| `symfonyParameter` | the first argument of `getParameter()` on a controller, of `getParameter()` and `hasParameter()` on a container, and of `get()` and `has()` on a parameter bag | its key under `parameters:` |
| `symfonyTranslation` | the first argument of `trans()` on a translator, of `new TranslatableMessage(…)` and of `t()` | the key in each locale's catalogue |
| `doctrineField` | the keys of the criteria of `findBy()`, `findOneBy()` and `count()` on a repository, and of the order of `findBy()` and `findOneBy()` | a property of the repository's entity |

What each one's usages and rename reach:

- **A route:** the PHP strings and the templates naming it in `path()` and `url()`. Renaming
  changes the part of the name written at the declaration, there and in every usage.
- **A template:** the PHP strings and the templates naming it (`{% extends %}`,
  `{% include %}`, …). Renaming changes the name in all of them and moves the file. Only a
  name without a namespace that ends in `.twig` is renamed: where a bundle's `@Namespace/…`
  template is can't be changed from the project.
- **A service or a parameter:** the PHP strings and what the YAML files below `config/`
  write: `@mailer` (and `@?mailer`) for a service, `%app.locale%` for a parameter. A service
  is also what follows `alias:` and `parent:`. Renaming changes the key and all of those. A
  service whose id is a class name isn't renamed here: that is the class's rename.
- **A translation key:** the PHP strings and a template's `'key'|trans`. Hover shows the
  translations. Renaming changes the key in every catalogue and every usage. A nested key is
  written in parts, so only its last part is renamed, to a name without a dot; a key that
  some catalogues nest and others write whole isn't renamed.
- **A field:** the keys naming the same field of the same entity. The entity is the one of
  the repository the method is called on: the one its mapping or its constructor names,
  looked for up the repository's parents.

Templates aren't PHP, so the editor's requests about them start in `crate::twig`; what the
project knows about the name at the caret is asked from `frameworks::symfony::refs` by the
kind's id (`PhpEngine::symfony_name_usages` and the methods beside it). A template with
unsaved changes is read as the editor has it.

### Names that name nothing

- `PhpUndefinedRoute`: the first argument of `path()`, `url()`, `generateUrl()`,
  `redirectToRoute()` and a URL generator's `generate()` that no route of
  `config/routes*.yaml` or `#[Route]` attribute has. Only string literals are checked, and
  only while the project names some route: when its routes come from somewhere the engine
  doesn't read, every name is taken as known.
- `PhpUndefinedTemplate`: a Twig template PHP names that the project's template folders
  don't have, reported when that is certain.

A service id that isn't defined is not reported: a service whose id is its class name needs
no definition (autowiring). A translation message that isn't a key is nothing wrong either,
as the text itself may be the message.

## Types

`SymfonyProvider` answers only where the declared and documented types don't name a class.

- **Container:** `$container->get(Foo::class)` (and `get('App\Foo')`) on a PSR or Symfony
  container is `Foo`: with autowiring, a service's id is its class name. An id the services
  files define is the class its definition makes, through aliases and parents
  (`get('mailer.default')`).
- **Repositories:** `getRepository(Product::class)` on an object manager or a manager
  registry is the repository class the entity names
  (`#[ORM\Entity(repositoryClass: ProductRepository::class)]`), else
  `EntityRepository<Product>`. On a repository of a known entity, `find()`, `findOneBy()` and
  the magic `findOneBy{Field}()` are `?Product`; `findAll()`, `findBy()` and
  `findBy{Field}()` are `list<Product>`; `count()` and `countBy{Field}()` are `int`. A
  repository without generics gets its entity from the entity whose mapping names it, or
  from its `parent::__construct($registry, Product::class)`.
- **Members:** on such a repository, `findBy{Field}`, `findOneBy{Field}` and
  `countBy{Field}` for each of the entity's properties.
- **Associations:** a property with `#[ORM\OneToMany(targetEntity: Comment::class)]` or
  `#[ORM\ManyToMany]` declared as a bare `Collection` is a `Collection<int, Comment>`; one
  with `#[ORM\ManyToOne]` or `#[ORM\OneToOne]` and no class in its type is `?Comment`.
  Wherever Doctrine's attributes are used, with or without the framework bundle.

Everything but the associations applies only in a Symfony project.

## Templates

`frameworks/symfony/templates.rs` says what PHP knows about a Twig template:

- **Its variables** (`PhpEngine::twig_template_variables`): the keys of the array given with
  the template's name (`$this->render('blog/show.html.twig', ['post' => $post])`), with the
  types of their values, from every place the project renders the template.
- **What follows a `.`** (`PhpEngine::twig_members`): on an object Twig looks for a public
  property, then a method of that name, then `getName()`, `isName()` and `hasName()`; on an
  array, for the key. So the members of `post.` are the public properties of its class and
  its public methods, the getters under the name Twig reads them by (`getTitle()` as
  `title`). A loop's variable (`{% for comment in post.comments %}`) is an element of what
  the loop goes through.

The Twig filters, functions and tests the project's own PHP adds, and those of an installed
package, are known in its templates too.

## Tests

- `crates/phoxite/src/symfony/tests.rs`: the configuration reader.
- `crates/phoxite/src/types/provider/symfony_tests.rs`: the container, the repositories and
  their finders.
- `crates/phoxite/src/navigate/tests/symfony.rs`: each kind's completion, goto, usages and
  rename, in PHP, in YAML and in templates; the two inspections; what a template is passed
  and what follows a dot.
- `crates/phoxite-lsp/src/tests.rs`: route names and Twig templates through the server, and
  the `frameworks.symfony` setting turning them off.
- `fixtures/symfony-app` is the project the VS Code end-to-end suite runs on; no Rust test
  reads it.

## What cannot be read

- A PHP configuration file that returns a closure configuring the container says nothing
  that can be read; only one returning an array is.
- Routes that come from somewhere other than the YAML routes files and the `#[Route]`
  attributes (a custom loader, a bundle's own routes). While the project names no route at
  all, route names are not reported.
- Services of a namespace loaded from a resource are known by their class alone, without
  what the configuration says of them.
- A bundle's `@Namespace/…` templates are found, but not renamed.
