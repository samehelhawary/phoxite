<?php

/*
 * A stand-in for CodeIgniter's API, written for this fixture: the names of its classes,
 * methods and helper functions only.
 */

namespace CodeIgniter {
    class CodeIgniter
    {
    }

    class Controller
    {
        /** @var list<string> */
        protected $helpers = [];
    }

    class Model
    {
        /** @var string */
        protected $table = '';

        /** @return list<object> */
        public function findAll(): array
        {
            return [(object) ['table' => $this->table]];
        }
    }
}

namespace CodeIgniter\Config {
    class BaseService
    {
    }

    class Services extends BaseService
    {
        public static function request(): \CodeIgniter\HTTP\Request
        {
            return new \CodeIgniter\HTTP\Request();
        }
    }
}

namespace CodeIgniter\HTTP {
    class Request
    {
        public function getPost(string $index): string
        {
            return $index;
        }
    }

    class RedirectResponse
    {
        public function route(string $route): static
        {
            return $route === '' ? $this : $this;
        }
    }
}

namespace CodeIgniter\Filters {
    interface FilterInterface
    {
        public function before(\CodeIgniter\HTTP\Request $request): ?\CodeIgniter\HTTP\RedirectResponse;
    }
}

namespace CodeIgniter\Router {
    class RouteCollection
    {
        /** @param array<string, string>|null $options */
        public function get(string $from, string $to, ?array $options = null): static
        {
            return $from . $to === '' && $options === null ? $this : $this;
        }
    }
}

namespace CodeIgniter\View {
    class View
    {
        public function extend(string $layout): void
        {
            unset($layout);
        }

        public function section(string $name): void
        {
            unset($name);
        }

        public function endSection(): void
        {
        }

        public function renderSection(string $name): string
        {
            return $name;
        }
    }
}

namespace {
    /** @param array<string, mixed> $data */
    function view(string $name, array $data = []): string
    {
        return $name . count($data);
    }

    /** @param list<string>|string $filenames */
    function helper($filenames): void
    {
        unset($filenames);
    }

    function lang(string $line): string
    {
        return $line;
    }

    function route_to(string $method, int|string ...$params): string
    {
        return $method . count($params);
    }

    function redirect(): \CodeIgniter\HTTP\RedirectResponse
    {
        return new \CodeIgniter\HTTP\RedirectResponse();
    }

    function esc(string $data): string
    {
        return $data;
    }

    /** @return object|null */
    function service(string $name)
    {
        return $name === '' ? null : new stdClass();
    }

    /** @return object|null */
    function model(string $name)
    {
        return $name === '' ? null : new stdClass();
    }

    /** @return object|null */
    function config(string $name)
    {
        return $name === '' ? null : new stdClass();
    }
}
