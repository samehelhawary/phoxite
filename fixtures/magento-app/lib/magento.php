<?php
// A stand-in for Magento's framework: names only.
namespace Magento\Framework {
    interface ObjectManagerInterface {
        public function create($type, array $arguments = []);
        public function get($type);
    }
    interface AuthorizationInterface {
        public function isAllowed($resource, $privilege = null);
    }
    class DataObject {
        public function __construct(array $data = []) {}
        public function getData($key = '', $index = null) {}
        public function setData($key, $value = null) {}
        public function __call($method, $args) {}
    }
    class Escaper {
        public function escapeHtml($data, $allowedTags = null) {}
        public function escapeUrl($string) {}
    }
}
namespace Magento\Framework\ObjectManager {
    class ObjectManager implements \Magento\Framework\ObjectManagerInterface {
        public function create($type, array $arguments = []) {}
        public function get($type) {}
    }
}
namespace Magento\Framework\Event {
    interface ManagerInterface {
        public function dispatch($eventName, array $data = []);
    }
    class Observer extends \Magento\Framework\DataObject {
        public function getEvent() {}
    }
    interface ObserverInterface {
        public function execute(Observer $observer);
    }
}
namespace Magento\Framework\App\Config {
    interface ScopeConfigInterface {
        public function getValue($path, $scopeType = 'default', $scopeCode = null);
        public function isSetFlag($path, $scopeType = 'default', $scopeCode = null);
    }
}
namespace Magento\Framework\Module {
    class Manager {
        public function isEnabled($moduleName) {}
    }
}
namespace Magento\Framework\Component {
    class ComponentRegistrar {
        const MODULE = 'module';
        public static function register($type, $componentName, $path) {}
    }
}
namespace Magento\Framework\Api {
    interface ExtensionAttributesInterface {}
    abstract class AbstractSimpleObject {}
}
namespace Magento\Framework\View {
    interface LayoutInterface {
        public function getBlock($name);
        public function getChildBlock($parentName, $alias);
    }
}
namespace Magento\Framework\View\Layout {
    interface ProcessorInterface {
        public function addHandle($handleName);
    }
}
namespace Magento\Framework\View\TemplateEngine {
    class Php {
        public function render($block, $fileName, array $dictionary = []) {}
    }
}
namespace Magento\Framework\View\Helper {
    class SecureHtmlRenderer {
        public function renderTag(string $tagName, array $attributes, ?string $content = null): string {}
    }
}
namespace Magento\Framework\View\Element {
    abstract class AbstractBlock extends \Magento\Framework\DataObject {
        public function getLayout() {}
        public function getChildBlock($alias) {}
        public function getChildHtml($alias = '', $useCache = true) {}
    }
    class Template extends AbstractBlock {
        protected $_template;
        public function setTemplate($template) {}
        public function getTemplate() {}
    }
}
namespace Magento\Backend\App {
    abstract class AbstractAction {
        const ADMIN_RESOURCE = 'Magento_Backend::admin';
        public function execute() {}
    }
    abstract class Action extends AbstractAction {}
}
namespace Magento\Framework {
    interface UrlInterface {
        public function getUrl($routePath = null, $routeParams = null);
    }
}
namespace Magento\Framework\App {
    class ResourceConnection {
        public function getTableName($modelEntity) {}
    }
}
namespace Magento\Framework\App\Action {
    abstract class Action {
        protected function _redirect($path, $arguments = []) {}
    }
}
namespace Magento\Framework\Model\ResourceModel\Db {
    abstract class AbstractDb {
        protected function _construct() {}
        protected function _init($mainTable, $idFieldName) {}
        public function getTable($tableName) {}
    }
}
