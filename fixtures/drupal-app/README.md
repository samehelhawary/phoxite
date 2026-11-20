# drupal-app

A small Drupal site for the engine's tests: a module (`modules/shop`) and a theme
(`themes/shelf`).

Nothing here is Drupal's. `core/` stands in for it: the names of the classes, methods and
hooks the module and the theme use, with empty bodies and docblocks written for this
repository. `crates/phoxite/tests/drupal_fixture.rs` opens the folder and checks that
nothing in the module or the theme is reported, and that its names resolve.
