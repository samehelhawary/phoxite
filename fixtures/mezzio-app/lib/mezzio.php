<?php

/*
 * A stand-in for Mezzio and the parts of Laminas it uses, written for this fixture:
 * the names of the classes and methods the application uses, and nothing of their code.
 */

namespace Psr\Container {
    interface ContainerInterface
    {
        /** @return mixed */
        public function get(string $id);

        public function has(string $id): bool;
    }
}

namespace Laminas\ConfigAggregator {
    class ConfigAggregator
    {
        public function __construct(array $providers = [])
        {
        }

        public function getMergedConfig(): array
        {
            return [];
        }
    }

    class PhpFileProvider
    {
        public function __construct(string $pattern)
        {
        }
    }
}

namespace Mezzio {
    class Application
    {
        /** @param mixed $middleware */
        public function get(string $path, $middleware, ?string $name = null): void
        {
        }

        /** @param mixed $middleware */
        public function post(string $path, $middleware, ?string $name = null): void
        {
        }
    }
}

namespace Mezzio\Template {
    interface TemplateRendererInterface
    {
        /** @param array|object $params */
        public function render(string $name, $params = []): string;
    }
}

namespace Mezzio\Helper {
    class UrlHelper
    {
        public function generate(?string $routeName = null, array $routeParams = []): string
        {
            return '/';
        }
    }
}

namespace Laminas\View\Renderer {
    class PhpRenderer
    {
        /** @return mixed */
        public function plugin(string $name, ?array $options = null)
        {
            return null;
        }

        /** @return mixed */
        public function __call(string $method, array $argv)
        {
            return null;
        }
    }
}

namespace Laminas\View {
    class HelperPluginManager
    {
        /** @var array<string, string> */
        protected $aliases = [
            'partial' => Helper\Partial::class,
        ];
    }
}

namespace Laminas\View\Helper {
    class Partial
    {
        /** @param array|object|null $values */
        public function __invoke(?string $name = null, $values = null): string
        {
            return (string) $name;
        }
    }
}

namespace Mezzio\LaminasView {
    class ConfigProvider
    {
        public function __invoke(): array
        {
            return [
                'view_helpers' => [
                    'aliases' => [
                        'url' => UrlHelper::class,
                    ],
                ],
            ];
        }
    }

    class UrlHelper
    {
        public function __invoke(?string $routeName = null, array $routeParams = []): string
        {
            return '/';
        }
    }
}
