<?php

/*
 * More of the stand-in for CodeIgniter's API, written for this fixture: events, validation
 * and the database. The names of its classes and methods only.
 */

namespace CodeIgniter\Events {
    class Events
    {
        public static function on(string $eventName, callable $callback): void
        {
            unset($eventName, $callback);
        }

        public static function trigger(string $eventName, mixed ...$arguments): bool
        {
            return $eventName !== '' && count($arguments) >= 0;
        }
    }
}

namespace CodeIgniter\Validation {
    class Validation
    {
        /**
         * @param array<string, mixed> $rules
         * @return $this
         */
        public function setRules(array $rules)
        {
            unset($rules);

            return $this;
        }

        /** @param array<string, mixed> $data */
        public function run(array $data, ?string $group = null): bool
        {
            return count($data) > 0 || $group !== null;
        }
    }

    class Rules
    {
        public function required(?string $value = null): bool
        {
            return $value !== null && $value !== '';
        }

        public function min_length(?string $value, string $length): bool
        {
            return strlen((string) $value) >= (int) $length;
        }
    }

    class FormatRules
    {
        public function valid_email(?string $value = null): bool
        {
            return str_contains((string) $value, '@');
        }
    }
}

namespace CodeIgniter\Database {
    class BaseBuilder
    {
        /** @return $this */
        public function where(string $key, mixed $value = null)
        {
            unset($key, $value);

            return $this;
        }

        /** @return $this */
        public function orderBy(string $orderBy)
        {
            unset($orderBy);

            return $this;
        }

        public function countAllResults(): int
        {
            return 0;
        }
    }

    class BaseConnection
    {
        public function table(string $tableName): BaseBuilder
        {
            unset($tableName);

            return new BaseBuilder();
        }
    }

    class Forge
    {
        /** @param array<string, array<string, mixed>> $fields */
        public function addField(array $fields): void
        {
            unset($fields);
        }

        public function createTable(string $table): bool
        {
            return $table !== '';
        }

        public function dropTable(string $tableName): bool
        {
            return $tableName !== '';
        }
    }

    abstract class Migration
    {
        protected Forge $forge;

        public function __construct()
        {
            $this->forge = new Forge();
        }

        abstract public function up(): void;

        abstract public function down(): void;
    }
}

namespace {
    function view_cell(string $library): string
    {
        return $library;
    }
}
