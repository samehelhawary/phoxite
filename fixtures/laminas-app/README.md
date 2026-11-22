# laminas-app

A small Laminas MVC application for the engine's tests, laid out like Laminas's MVC
skeleton: a module with its configuration (routes with a child route, services, a
controller plugin, a view helper, a template map and a template path), a controller with
its factory, services (one made by a closure), an input filter specification, and
templates with a layout the controller gives a variable, a partial and a child view.

Nothing here is Laminas's. `lib/` stands in for it: the names of the classes and methods
the application uses, written for this fixture. `composer.json` names
`laminas/laminas-mvc`, which is what makes the project a Laminas one.
