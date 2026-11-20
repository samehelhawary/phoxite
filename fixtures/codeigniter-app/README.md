# codeigniter-app

A small CodeIgniter 4 application for the engine's tests: a controller, a model, a
library, a filter, a language file, a helper and two views.

Nothing here is CodeIgniter's. `system/` stands in for it: the names of the classes,
methods and helper functions the application uses, with bodies and docblocks written for
this repository. `crates/phoxite/tests/codeigniter_fixture.rs` opens the folder and checks
that nothing in `app/` is reported, and that its names resolve.
