# WordPress

What the engine knows about a WordPress project, where the code is, and what it doesn't
do. The user-facing list is in `editors/vscode/README.md`; how a framework plugs into the
engine is in [`frameworks.md`](frameworks.md).

## Nothing is bundled

WordPress is GPL and this engine is Apache-2.0. The engine has no copy of WordPress's
code, of its documentation or of a list of its hooks. It knows two things:

- the names and argument positions of the functions of WordPress's public API that take
  a name (`add_action`, `register_post_type`, …), as the developer handbooks document
  them;
- whatever the files it indexes say: WordPress's own source when the project has it, a
  stubs package in `vendor/` (`php-stubs/wordpress-stubs`), or a folder named in
  `includePaths`.

A plugin or theme opened on its own, with none of those, still has the hooks it fires
itself, but `add_action` is then an undeclared function to the engine.

## Detection

`frameworks/detect.rs`: `wp-config.php`, `wp-load.php` or `wp-includes/version.php` in the
project; a PHP file at its root with a `Plugin Name:` header, or a `style.css` with a
`Theme Name:` header; or Composer installing WordPress or its stubs. The `frameworks`
setting forces it on or off.

## Names

A hook exists where `do_action('name')` fires it, a post type where
`register_post_type('name')` registers it: there is no PHP declaration to index. The
summary of every file records the calls that declare such a name (`summary/names.rs`,
`summary/extract/names.rs`): what it is (`DeclaredKind`), the name, and where its string
is. They are recorded in any project, by the called function's name alone; the index
lists the files that have some, and `PhpEngine::names()` (`frameworks/names.rs`) puts
them together when a WordPress reference kind asks.

The usages aren't facts. They are found like those of every string reference
(`frameworks/refs`): the word index gives the files that write the name, and each
string there is checked against the kind's patterns.

| Kind | Declared by | Named in |
| --- | --- | --- |
| `wordpressHook` | `do_action`, `apply_filters`, their `_ref_array` and `_deprecated` forms | `add_action`, `add_filter`, `remove_action`, `remove_filter`, `remove_all_actions`, `remove_all_filters`, `has_action`, `has_filter`, `did_action`, `did_filter`, `doing_action`, `doing_filter` |
| `wordpressShortcode` | `add_shortcode` | `remove_shortcode`, `shortcode_exists`, `has_shortcode` |
| `wordpressPostType` | `register_post_type` | `get_post_type_object`, `post_type_exists`, `is_singular`, `is_post_type_archive`, `'post_type' =>` of `get_posts`, `new WP_Query` and `wp_insert_post`, the object type of `register_taxonomy`, … |
| `wordpressTaxonomy` | `register_taxonomy` | `get_terms(['taxonomy' => …])`, `get_the_terms`, `has_term`, `is_tax`, `wp_set_object_terms`, … |
| `wordpressScript`, `wordpressStyle` | `wp_register_script`, and `wp_enqueue_script` given a source (and `…_style`) | `wp_enqueue_script`, `wp_localize_script`, `wp_add_inline_script`, the dependencies of another, … |
| `wordpressOption` | `add_option`, `update_option`, `register_setting` | `get_option`, `delete_option` |
| `wordpressImageSize` | `add_image_size` | `the_post_thumbnail`, `wp_get_attachment_image`, … |
| `wordpressNavMenu` | `register_nav_menu`, the keys of `register_nav_menus` | `wp_nav_menu(['theme_location' => …])`, `has_nav_menu` |
| `wordpressBlock` | `register_block_type('namespace/name')`, the `name` of a `block.json` | `has_block`, `unregister_block_type`, `register_block_style`, `WP_Block_Type_Registry::is_registered`; a script's `'namespace/name'` |
| `wordpressRestRoute` | `register_rest_route('shop/v1', '/books')` | `rest_url`, `get_rest_url`, `rest_do_request`, `new WP_REST_Request`; a script's `'/shop/v1/books'` |
| `wordpressTemplatePart` | the theme's file | `get_template_part` |
| `wordpressThemeFile` | the theme's file | `get_header`, `get_footer`, `get_sidebar`, `locate_template`, `get_theme_file_path`, `get_theme_file_uri` |

All but the last three are one type, `WordPressName` (`frameworks/wordpress/refs/names.rs`),
with completion, goto, Find References, hover and rename. A new registry is a
`DeclaredKind`, a row of the extraction table and a `WordPressName`.

- **Actions and filters are one kind.** WordPress keeps them in one table (`add_action`
  is `add_filter`).
- **A list of names** where a function takes one or several: each item of
  `is_singular(['book', 'page'])`, of `'post_type' => ['book', 'page']` in a query, of the
  post types of `register_taxonomy()` and of `'taxonomies' => […]` of a post type
  (`Position::ListOfKey` for the keyed ones).
- **Blocks** are the ones PHP registers by name and the ones a `block.json` declares
  (`frameworks/wordpress/assets.rs`; the engine keeps the list until a `block.json`
  changes, which the server watches). A rename changes the `block.json` too. A literal
  `register_block_type('build/price')` is a folder when there is one of that name beside
  the file, and a block's name otherwise.
- **REST routes** (`refs/rest.rs`) are a namespace and a route where they are declared
  and one path where they are named: `shop/v1/books`. A slash before it and a query
  after it aren't part of it. A route with a pattern (`/books/(?P<id>\d+)`) is recorded as
  `shop/v1/books/*`, and of several that fit a path those with as many parts come first.
  Find References works from the route's string too. The namespace and the route may be
  properties of the class set to one string (`$this->namespace`, `$this->rest_base`, as
  WordPress's controllers write them); a part that isn't known is a `*`, and a route
  whose namespace isn't known is left out. Rename changes the last part of the path
  (`books`): in the route's string, in the routes that go on from it, and in every PHP
  string and script writing them (`site_rename_range`, `other_renames`).
- **Scripts** (`assets.rs`). The engine doesn't parse JavaScript: Find References and
  Rename search the project's `.js`, `.jsx`, `.mjs`, `.ts` and `.tsx` files as text for
  the name in quotes. A block's name; a REST route's path (also as the end of a longer
  URL); and for a `wp_ajax_{action}` or `wp_ajax_nopriv_{action}` hook, the action on a
  line that says `action` or after `action=`. Not read: WordPress's folders,
  `node_modules`, `vendor`, minified files, files over 512 KB. Renaming such a hook
  renames the action: in both hooks and in the scripts.
- **In a script or a `block.json`** (`frameworks/wordpress/scripts.rs`, the server's
  `Kind::WordPressScript`): goto, hover, Find References and Rename on the string at the
  caret when it names a block or a REST route the project declares, or (on a line saying
  `action`) an AJAX action, which goes to the calls listening to its hooks. In a
  `block.json`, `"file:./render.php"` goes to the file. Completion goes by the text
  before the string: `action` offers the actions PHP listens to with a named callback,
  `path`, `fetch` or a leading slash the routes, `Block` or `name` the blocks. No
  client lists these files for the server: while a WordPress project is open the server
  registers for them (`Server::register_scripts`: their documents and these requests),
  and unregisters when none is. A client without dynamic registration has to be set up
  to send them.
- **A name put together at run time** (`"save_post_{$post->post_type}"`, `'manage_' .
  $type . '_columns'`) is recorded as a pattern, `save_post_*`. A string that fits it
  goes there and isn't unknown. Patterns aren't offered by completion or renamed.
- **Rename** is for names the project declares itself: not when a declaration is in a
  library, in `wp-includes` or `wp-admin`, or in a file beside those two folders
  (`wp-settings.php` fires `init`). The declaring calls are string sites of the kind
  too, so a rename changes them with the rest.
- **`PhpUndefinedHook`** reports a hook listened to that nothing indexed fires. It is one
  of `inspect::OPT_IN`: off until a setting turns it on, since a plugin that isn't in
  the workspace may fire the hook. The other kinds have no such inspection: an option
  may only be in the database, and the built-in image sizes are registered nowhere.
- **Template parts** (`refs/template_part.rs`): `get_template_part($slug, $name)` is
  `{slug}-{name}.php`, else `{slug}.php`, in the theme: the nearest folder above the
  calling file with a `style.css` or a `functions.php`. A child theme's files come
  first, then its parent's: the folder beside it that the `Template:` header of its
  `style.css` names.
- **Theme files** (`ThemeFile`, same file): `get_header('shop')` is `header-shop.php`,
  else `header.php` (and so for the footer and the sidebar); `locate_template()` (one
  path or a list) and `get_theme_file_path()` and `…_uri()` name a file of the theme by
  its path, of any type. A path that leaves the theme names nothing.

## Callbacks

A hook's callback is a string callable (`navigate/callable.rs`), in any project:
`add_action()`'s docblock says `@param callable $callback`. Callbacks given under a key
of an array aren't typed by anything WordPress writes (it documents those arrays in
prose). They are told by the key's name: one that says so (`callback`, `…_callback`,
`…_cb`) holds a callable in any array, wherever it is written: `callback` and
`permission_callback` of `register_rest_route`, `sanitize_callback` of
`register_setting`, `render_callback` of `register_block_type`,
`$wp_customize->add_setting($id, ['sanitize_callback' => 'absint'])`, and an array
assigned to a variable first.

## Types

Both come from docblocks, and both are answered by the resolver from the name facts
(`types/resolve.rs`), not by a type provider:

- **What a hook passes its callbacks** (`Resolver::hook_parameters`). WordPress
  documents a hook in the docblock in front of the statement firing it; the `@param`
  types there, in order, are recorded with the firing call (`DeclaredName::params`). A
  closure or arrow function given to `add_action` or `add_filter` gets them as the types
  its parameters are expected to have (`types/expr/flow.rs`); a function or method
  registered by name (`'shop_saved'`, `[$this, 'saved']`, `[Foo::class, 'saved']`) gets
  them for the parameters nothing else types (`NameFacts::callbacks`,
  `Resolver::hook_callback_parameter`), from every hook it is registered for.
- **Global variables** (`Resolver::global_type`). `@global wpdb $wpdb` on a function says
  what the global is. `global $wpdb;` has the type its own function documents, else the
  type most `@global` tags of the project give that name. The same type is that of
  `$GLOBALS['wpdb']`, and of a variable a file reads at its top level without setting it
  (a theme's template reads `$post`). This is in every PHP project: the tag is
  phpDocumentor's.

## Tests

- `navigate/tests/wordpress.rs`: every feature, on a stand-in for WordPress's API written
  for the tests (function names and parameters, none of its code).
- `phoxite-lsp/src/tests.rs`: a plugin opened on its own and one of its scripts, through
  the protocol; that the scripts are asked for in a WordPress project and not in a
  Laravel one.
- `editors/vscode/test/e2e` runs once on the fixture site in VS Code (`wordpressChecks`
  in `suite.js`; `PHOXITE_E2E_ONLY=wordpress` runs it alone): hooks (goto, hover,
  completion, usages, rename), callbacks, the types of a callback's parameters and of the
  globals, each registry, the theme's files, REST routes, a script, a `block.json`, and
  the hook inspection turned on by its setting.
- `tests/wordpress_fixture.rs` on `fixtures/wordpress-app`, a plugin and a theme on a
  stand-in API: nothing in them is reported (the hook inspection turned on), and one name
  of each kind goes where it should.
- `tests/wordpress_corpus.rs` (ignored; `scripts/php-corpus.sh wordpress`): WordPress
  itself. That its hooks are found where it fires them (about 3,500 firing calls, 300 of
  them under a name made at run time), that its docblocks type a callback and its
  globals, and that a plugin can't rename what WordPress declares.

## Not done

- A script isn't parsed: a name it puts together (`'shop/v1/' + kind`) isn't found, and
  a string is told by its line alone. A rename reads the scripts as they are saved.
- A REST route whose namespace comes from anywhere but a literal or a property set to
  one string.
- A callback under a key that isn't named as one: nothing says
  such a string is a function's name.
