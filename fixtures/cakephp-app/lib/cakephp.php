<?php

/*
 * A stand-in for CakePHP, written for this fixture: the names of the classes and
 * methods the application uses, and nothing of CakePHP's code. It documents no
 * generics.
 */

namespace {
    /** @param mixed ...$args */
    function __(string $singular, ...$args): string
    {
        return $singular;
    }
}

namespace Migrations {
    class Table
    {
        /** @return $this */
        public function addColumn(string $columnName, ?string $type = null, array $options = [])
        {
            return $this;
        }

        public function create(): void
        {
        }
    }

    class AbstractMigration
    {
        public function table(string $tableName, array $options = []): Table
        {
            return new Table();
        }
    }
}

namespace Cake\Datasource {
    interface EntityInterface
    {
    }

    interface QueryInterface
    {
    }

    interface ResultSetInterface
    {
    }
}

namespace Cake\Core {
    class Configure
    {
        /** @return mixed */
        public static function read(?string $var = null, $default = null)
        {
            return $default;
        }

        public static function check(string $var): bool
        {
            return $var !== '';
        }
    }

    class BasePlugin
    {
    }
}

namespace Cake\ORM\Locator {
    interface LocatorInterface
    {
        public function get(string $alias, array $options = []): \Cake\ORM\Table;
    }

    class TableLocator implements LocatorInterface
    {
        public function get(string $alias, array $options = []): \Cake\ORM\Table
        {
            return new \Cake\ORM\Table();
        }
    }

    trait LocatorAwareTrait
    {
        public function getTableLocator(): LocatorInterface
        {
            return new TableLocator();
        }

        public function fetchTable(?string $alias = null, array $options = []): \Cake\ORM\Table
        {
            return $this->getTableLocator()->get((string) $alias, $options);
        }
    }
}

namespace Cake\ORM {
    use Cake\Datasource\EntityInterface;

    class Entity implements EntityInterface
    {
        public function isNew(): bool
        {
            return false;
        }

        /** @return mixed */
        public function __get(string $field)
        {
            return null;
        }
    }

    class Behavior
    {
    }

    class Association
    {
        public function getTarget(): Table
        {
            return new Table();
        }

        /** @return mixed */
        public function __call(string $method, array $arguments)
        {
            return null;
        }
    }

    class Table
    {
        public function initialize(array $config): void
        {
        }

        /** @return $this */
        public function setTable(string $table)
        {
            return $this;
        }

        /** @return $this */
        public function setEntityClass(string $name)
        {
            return $this;
        }

        /** @return $this */
        public function addBehavior(string $name, array $options = [])
        {
            return $this;
        }

        public function getBehavior(string $name): Behavior
        {
            return new Behavior();
        }

        public function belongsTo(string $associated, array $options = []): Association\BelongsTo
        {
            return new Association\BelongsTo();
        }

        public function hasOne(string $associated, array $options = []): Association\HasOne
        {
            return new Association\HasOne();
        }

        public function hasMany(string $associated, array $options = []): Association\HasMany
        {
            return new Association\HasMany();
        }

        public function belongsToMany(string $associated, array $options = []): Association\BelongsToMany
        {
            return new Association\BelongsToMany();
        }

        public function find(string $type = 'all'): Query\SelectQuery
        {
            return new Query\SelectQuery();
        }

        /** @param mixed $primaryKey */
        public function get($primaryKey): EntityInterface
        {
            return new Entity();
        }

        public function newEmptyEntity(): EntityInterface
        {
            return new Entity();
        }

        public function newEntity(array $data, array $options = []): EntityInterface
        {
            return new Entity();
        }

        public function patchEntity(EntityInterface $entity, array $data, array $options = []): EntityInterface
        {
            return $entity;
        }

        /** @return \Cake\Datasource\EntityInterface|false */
        public function save(EntityInterface $entity, array $options = [])
        {
            return $entity;
        }

        /** @return \Cake\ORM\Association */
        public function __get(string $property)
        {
            return new Association();
        }
    }
}

namespace Cake\ORM\Association {
    class BelongsTo extends \Cake\ORM\Association
    {
    }

    class HasOne extends \Cake\ORM\Association
    {
    }

    class HasMany extends \Cake\ORM\Association
    {
    }

    class BelongsToMany extends \Cake\ORM\Association
    {
    }
}

namespace Cake\ORM\Behavior {
    class TimestampBehavior extends \Cake\ORM\Behavior
    {
    }
}

namespace Cake\ORM\Query {
    class SelectQuery implements \Cake\Datasource\QueryInterface
    {
        /**
         * @param mixed $conditions
         * @return $this
         */
        public function where($conditions)
        {
            return $this;
        }

        /**
         * @param mixed $associations
         * @return $this
         */
        public function contain($associations)
        {
            return $this;
        }

        /** @return mixed */
        public function first()
        {
            return null;
        }

        public function toArray(): array
        {
            return [];
        }
    }
}

namespace Cake\Controller {
    class Component
    {
    }

    class Controller
    {
        use \Cake\ORM\Locator\LocatorAwareTrait;

        public function initialize(): void
        {
        }

        public function loadComponent(string $name, array $config = []): Component
        {
            return new Component();
        }

        /**
         * @param array|string $name
         * @param mixed $value
         * @return $this
         */
        public function set($name, $value = null)
        {
            return $this;
        }

        public function viewBuilder(): \Cake\View\ViewBuilder
        {
            return new \Cake\View\ViewBuilder();
        }

        public function render(?string $template = null, ?string $layout = null): \Cake\Http\Response
        {
            return new \Cake\Http\Response();
        }

        /** @param array|string $url */
        public function redirect($url, int $status = 302): ?\Cake\Http\Response
        {
            return new \Cake\Http\Response();
        }

        /** @return mixed */
        public function __get(string $name)
        {
            return null;
        }
    }
}

namespace Cake\Controller\Component {
    class FlashComponent extends \Cake\Controller\Component
    {
        public function success(string $message): void
        {
        }
    }
}

namespace Cake\Http {
    class Response
    {
    }
}

namespace Cake\Routing {
    class Router
    {
        /** @param array|string|null $url */
        public static function url($url = null, bool $full = false): string
        {
            return '/';
        }
    }

    class RouteBuilder
    {
        /**
         * @param array|string $defaults
         * @return $this
         */
        public function connect(string $route, $defaults = [], array $options = [])
        {
            return $this;
        }

        /**
         * @param \Closure|array $params
         * @return $this
         */
        public function scope(string $path, $params, ?\Closure $callback = null)
        {
            return $this;
        }
    }
}

namespace Cake\View {
    class Helper
    {
    }

    class ViewBuilder
    {
        /** @return $this */
        public function setTemplate(?string $name)
        {
            return $this;
        }

        /** @return $this */
        public function setLayout(?string $name)
        {
            return $this;
        }
    }

    class View
    {
        public function initialize(): void
        {
        }

        public function loadHelper(string $name, array $config = []): Helper
        {
            return new Helper();
        }

        public function element(string $name, array $data = [], array $options = []): string
        {
            return $name;
        }

        public function fetch(string $name, string $default = ''): string
        {
            return $default;
        }

        public function cell(string $cell, array $data = [], array $options = []): Cell
        {
            return new Cell();
        }

        /** @return \Cake\View\Helper|null */
        public function __get(string $name)
        {
            return null;
        }
    }

    class Cell
    {
        use \Cake\ORM\Locator\LocatorAwareTrait;

        /**
         * @param array|string $name
         * @param mixed $value
         * @return $this
         */
        public function set($name, $value = null)
        {
            return $this;
        }

        public function __toString(): string
        {
            return '';
        }
    }
}

namespace Cake\View\Helper {
    class HtmlHelper extends \Cake\View\Helper
    {
        /**
         * @param array|string $title
         * @param array|string|null $url
         */
        public function link($title, $url = null, array $options = []): string
        {
            return (string) $url;
        }
    }
}
