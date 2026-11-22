# magento-app

A small Magento 2 project for Phoxite's tests: one module, `Shop_Catalog`, under
`app/code`, and a theme under `app/design` that overrides one of its templates.

`lib/magento.php` is a stand-in for the framework: the names of the classes and methods
the module uses, nothing of Magento's code. `tests/magento_corpus.rs` runs the same
module on the real framework.

The module has no `generated/` folder: `CartFactory`, `Cart\Proxy` and
`ItemExtensionInterface` are the classes Magento would generate, which the engine
provides.
