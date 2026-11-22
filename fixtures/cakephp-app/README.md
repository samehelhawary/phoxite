# cakephp-app

A small CakePHP application for the engine's tests, laid out like CakePHP's application
skeleton: a configuration, routes with a named one, controllers with components, tables
with associations and a behaviour, entities, a migration making their tables, a
translation catalogue, a view class with a helper, a cell,
templates with a layout and an element, and a plugin with a table, a controller and a
template of its own.

Nothing here is CakePHP's. `lib/` stands in for it: the names of the classes and methods
the application uses, written for this fixture, without the generics CakePHP 5 documents
(as in CakePHP 4), so that what a table gives is the engine's to work out.
`composer.json` names `cakephp/cakephp`, which is what makes the project a CakePHP one.
