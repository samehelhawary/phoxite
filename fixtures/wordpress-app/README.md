# wordpress-app

A small WordPress site for the engine's tests: a plugin (`wp-content/plugins/shop`) and a
theme (`wp-content/themes/shelf`).

Nothing here is WordPress's. `wp-includes/api.php` stands in for it: the names and
parameters of the API functions the plugin and the theme call, with empty bodies and
docblocks written for this repository. `crates/phoxite/tests/wordpress_fixture.rs` opens
the folder and checks that nothing in it is reported, and that its names resolve.
