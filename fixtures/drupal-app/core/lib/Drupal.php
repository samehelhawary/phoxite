<?php

/*
 * A stand-in for Drupal's API, written for this fixture: the names of its classes and
 * methods only.
 */

namespace {
    class Drupal
    {
        public static function service(string $id): mixed
        {
            return $id;
        }

        public static function entityTypeManager(): \Drupal\Core\Entity\EntityTypeManagerInterface
        {
            return self::service('entity_type.manager');
        }

        public static function moduleHandler(): \Drupal\Core\Extension\ModuleHandlerInterface
        {
            return self::service('module_handler');
        }

        public static function config(string $name): \Drupal\Core\Config\Config
        {
            return new \Drupal\Core\Config\Config($name);
        }
    }
}

namespace Psr\Container {
    interface ContainerInterface
    {
        public function get(string $id): mixed;

        public function has(string $id): bool;
    }
}

namespace Drupal\Core\Config {
    class Config extends ConfigBase
    {
        public function __construct(private string $name)
        {
        }

        public function get(string $key = ''): mixed
        {
            return $this->name . $key;
        }
    }
}

namespace Drupal\Core {
    class Url
    {
        /** @param array<string, mixed> $route_parameters */
        public static function fromRoute(string $route_name, array $route_parameters = []): static
        {
            return new static($route_name, $route_parameters);
        }

        /** @param array<string, mixed> $route_parameters */
        final public function __construct(private string $route_name, private array $route_parameters = [])
        {
        }

        public function toString(): string
        {
            return $this->route_name . count($this->route_parameters);
        }
    }

    class Link
    {
        public static function createFromRoute(string $text, string $route_name): string
        {
            return $text . $route_name;
        }
    }
}

namespace Drupal\Core\Controller {
    class ControllerBase
    {
        protected function redirect(string $route_name): string
        {
            return $route_name;
        }

        protected function config(string $name): \Drupal\Core\Config\Config
        {
            return new \Drupal\Core\Config\Config($name);
        }
    }
}

namespace Drupal\Core\Session {
    interface AccountInterface
    {
        public function hasPermission(string $permission): bool;
    }
}

namespace Drupal\Core\Access {
    class AccessResult
    {
        public static function allowedIfHasPermission(\Drupal\Core\Session\AccountInterface $account, string $permission): bool
        {
            return $account->hasPermission($permission);
        }
    }
}

namespace Drupal\Core\Form {
    interface FormStateInterface
    {
        public function getValue(string $key): mixed;

        public function setRedirect(string $route_name): static;
    }
}

namespace Drupal\Core\Extension {
    interface ModuleHandlerInterface
    {
        public function moduleExists(string $module): bool;

        /**
         * @param array<int, mixed> $args
         * @return array<int, mixed>
         */
        public function invokeAll(string $hook, array $args = []): array;

        public function alter(string $type, mixed &$data): void;
    }
}

namespace Drupal\Component\Plugin {
    interface PluginManagerInterface
    {
        public function createInstance(string $plugin_id): object;
    }
}

namespace Drupal\Core\Block {
    interface BlockManagerInterface extends \Drupal\Component\Plugin\PluginManagerInterface
    {
    }

    abstract class BlockBase
    {
        /** @return array<string, mixed> */
        abstract public function build(): array;
    }
}

namespace Drupal\Core\Entity {
    interface EntityInterface
    {
        public function id(): int|string|null;

        public function label(): string;
    }

    interface EntityStorageInterface
    {
        public function load(int|string $id): ?EntityInterface;

        /** @return array<int|string, EntityInterface> */
        public function loadMultiple(): array;

        /** @param array<string, mixed> $values */
        public function create(array $values = []): EntityInterface;
    }

    interface EntityTypeManagerInterface extends \Drupal\Component\Plugin\PluginManagerInterface
    {
        public function getStorage(string $entity_type_id): EntityStorageInterface;
    }
}

namespace Drupal\Core\Hook\Attribute {
    #[\Attribute(\Attribute::TARGET_METHOD | \Attribute::IS_REPEATABLE)]
    class Hook
    {
        public function __construct(public string $hook)
        {
        }
    }
}

namespace Drupal\Core\Entity\Attribute {
    #[\Attribute(\Attribute::TARGET_CLASS)]
    class ContentEntityType
    {
        /** @param array<string, string> $handlers */
        public function __construct(public string $id, public string $label = '', public array $handlers = [])
        {
        }
    }
}

namespace Drupal\Core\Block\Attribute {
    #[\Attribute(\Attribute::TARGET_CLASS)]
    class Block
    {
        public function __construct(public string $id, public string $admin_label = '')
        {
        }
    }
}

namespace Drupal\Core\Config {
    abstract class ConfigBase
    {
    }
}
