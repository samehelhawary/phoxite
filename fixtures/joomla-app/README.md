# joomla-app

A small Joomla site for Phoxite's tests: one component, `com_shop`, with its site and
administrator folders, a content plugin, a module, and a site template that overrides a
layout and a view.

`libraries/joomla.php` is a stand-in for Joomla: the names of the classes and methods the
extensions use, nothing of Joomla's code. `libraries/src/Version.php` is what says the
folder is a Joomla site, and `libraries/src/Event/CoreEventAware.php` holds the map of
event names to event classes, shaped as Joomla's is.
