<?php
// A stand-in for Joomla and its framework packages: names only.
namespace Joomla\CMS {
    abstract class Factory {
        /** @return Application\CMSApplicationInterface */
        public static function getApplication() {}
    }
}
namespace Joomla\CMS\Application {
    interface CMSApplicationInterface extends \Joomla\CMS\Extension\ExtensionManagerInterface {
        /** @return \Joomla\CMS\User\User|null */
        public function getIdentity();
    }
    abstract class CMSApplication implements CMSApplicationInterface {
        public function getIdentity() {}
        /** @return \Joomla\CMS\Document\HtmlDocument */
        public function getDocument() {}
        public function bootComponent($component): \Joomla\CMS\Extension\ComponentInterface {}
    }
}
namespace Joomla\CMS\Document {
    class HtmlDocument {
        public function getWebAssetManager(): \Joomla\CMS\WebAsset\WebAssetManager {}
        public function countModules($condition, $withContentOnly = false): int {}
    }
}
namespace Joomla\CMS\Extension {
    interface ComponentInterface {}
    interface ExtensionManagerInterface {
        public function bootComponent($component): ComponentInterface;
    }
    class MVCComponent implements ComponentInterface {
        public function getMVCFactory(): \Joomla\CMS\MVC\Factory\MVCFactoryInterface {}
    }
}
namespace Joomla\CMS\MVC\Factory {
    interface MVCFactoryInterface {
        public function createModel($name, $prefix = '', array $config = []);
        public function createTable($name, $prefix = '', array $config = []);
    }
}
namespace Joomla\CMS\MVC\Controller {
    class BaseController {
        public function getModel($name = '', $prefix = '', $config = []) {}
        public function display($cachable = false, $urlparams = []) {}
    }
}
namespace Joomla\CMS\MVC\Model {
    class BaseDatabaseModel {}
    class FormModel extends BaseDatabaseModel {
        protected function loadForm($name, $source = null, $options = [], $clear = false, $xpath = null) {}
    }
}
namespace Joomla\CMS\MVC\View {
    abstract class AbstractView {
        public function getModel($name = null) {}
        public function get($property, $default = null) {}
    }
    class HtmlView extends AbstractView {
        public function display($tpl = null) {}
        public function loadTemplate($tpl = null): string {}
        public function escape($var): string {}
    }
}
namespace Joomla\CMS\Table {
    abstract class Table {
        public function __construct($table, $key, $db) {}
        public function load($keys = null, $reset = true) {}
    }
}
namespace Joomla\CMS\Language {
    class Text {
        public static function _($string, $jsSafe = false): string {}
        public static function sprintf($string): string {}
    }
}
namespace Joomla\CMS\Layout {
    class LayoutHelper {
        public static function render($layoutFile, $displayData = null, $basePath = '', $options = null): string {}
    }
    class FileLayout {
        public function __construct($layoutId, $basePath = null, $options = null) {}
        public function render($displayData = []): string {}
        public function escape($output): string {}
    }
}
namespace Joomla\CMS\HTML {
    abstract class HTMLHelper {
        public static function _(string $key, ...$methodArgs) {}
    }
}
namespace Joomla\CMS\HTML\Helpers {
    abstract class Behavior {
        public static function core() {}
    }
}
namespace Joomla\CMS\WebAsset {
    class WebAssetManager {
        public function __call($method, $arguments) {}
        public function useAsset(string $type, string $name) {}
    }
}
namespace Joomla\CMS\User {
    class User {
        public function authorise($action, $assetname = null): bool {}
    }
}
namespace Joomla\CMS\Form {
    class Form {
        public function getValue($name, $group = null, $default = null) {}
    }
    class FormField {}
}
namespace Joomla\CMS\Form\Field {
    class TextField extends \Joomla\CMS\Form\FormField {}
    class RadioField extends \Joomla\CMS\Form\FormField {}
}
namespace Joomla\CMS\Router {
    class Route {
        public static function _($url, $xhtml = true): string {}
    }
}
namespace Joomla\CMS\Plugin {
    abstract class CMSPlugin implements \Joomla\Event\SubscriberInterface {
        /** @return \Joomla\CMS\Application\CMSApplicationInterface */
        protected function getApplication() {}
    }
}
namespace Joomla\CMS\Event\Content {
    class ContentPrepareEvent extends \Joomla\Event\Event {
        public function getItem(): object {}
    }
    class AfterSaveEvent extends \Joomla\Event\Event {
        public function getIsNew(): bool {}
    }
}
namespace Joomla\Event {
    class Event {
        public function getName(): string {}
    }
    interface SubscriberInterface {
        public static function getSubscribedEvents(): array;
    }
    interface DispatcherInterface {
        public function dispatch(string $name, ?Event $event = null);
    }
}
namespace Joomla\Registry {
    class Registry {
        public function get($path, $default = null) {}
    }
}
namespace Joomla\Database {
    interface QueryInterface {
        public function select($columns);
        public function from($tables);
    }
    interface DatabaseInterface {
        public function getQuery($new = false): QueryInterface;
    }
}
namespace Joomla\Input {
    class Input {
        public function getInt($name, $default = null): int {}
    }
}
