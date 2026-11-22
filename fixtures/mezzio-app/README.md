# mezzio-app

A small Mezzio application for the engine's tests, laid out like Mezzio's skeleton: the
configuration aggregated from a `ConfigProvider` and the files of
`config/autoload`, named routes in `config/routes.php`, a handler with its factory,
and templates in a namespace.

Nothing here is Mezzio's or Laminas's. `lib/` stands in for them: the names of the
classes and methods the application uses, written for this fixture.
`composer.json` names `mezzio/mezzio`, which is what makes the project a Mezzio one.
