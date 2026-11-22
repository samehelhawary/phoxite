<?php

/*
 * A stand-in for Laminas, written for this fixture: the names of the classes and
 * methods the application uses, and nothing of Laminas's code.
 */

namespace Psr\Container {
    interface ContainerInterface
    {
        /** @return mixed */
        public function get(string $id);

        public function has(string $id): bool;
    }
}

namespace Laminas\ServiceManager\Factory {
    class InvokableFactory
    {
    }
}

namespace Laminas\Mvc\Controller {
    abstract class AbstractController
    {
        /** @return mixed */
        public function plugin(string $name, ?array $options = null)
        {
            return null;
        }

        /** @return mixed */
        public function __call(string $method, array $params)
        {
            return null;
        }
    }

    abstract class AbstractActionController extends AbstractController
    {
    }

    class PluginManager
    {
        /** @var array<string, string> */
        protected $aliases = [
            'layout' => Plugin\Layout::class,
            'redirect' => Plugin\Redirect::class,
            'url' => Plugin\Url::class,
        ];
    }
}

namespace Laminas\Mvc\Controller\Plugin {
    class Layout
    {
        public function __invoke(?string $template = null): Layout
        {
            return $this;
        }

        /**
         * @param mixed $value
         * @return $this
         */
        public function setVariable(string $name, $value)
        {
            return $this;
        }
    }

    class Redirect
    {
        public function toRoute(?string $route = null, array $params = [], array $options = []): \Laminas\Http\Response
        {
            return new \Laminas\Http\Response();
        }
    }

    class Url
    {
        public function fromRoute(?string $route = null, array $params = [], array $options = []): string
        {
            return '/';
        }
    }
}

namespace Laminas\Http {
    class Response
    {
    }
}

namespace Laminas\View\Model {
    class ViewModel
    {
        /**
         * @param array|null $variables
         * @param array|null $options
         */
        public function __construct($variables = null, $options = null)
        {
        }

        /**
         * @param mixed $value
         * @return $this
         */
        public function setVariable(string $name, $value)
        {
            return $this;
        }

        /** @return $this */
        public function setTemplate(string $template)
        {
            return $this;
        }

        /** @return $this */
        public function addChild(ViewModel $child, ?string $captureTo = null)
        {
            return $this;
        }
    }
}

namespace Laminas\Validator {
    class ValidatorPluginManager
    {
        /** @var array<string, string> */
        protected $aliases = [
            'stringlength' => StringLength::class,
            'StringLength' => StringLength::class,
        ];
    }

    class StringLength
    {
        public function isValid(string $value): bool
        {
            return $value !== '';
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

        /** @return mixed */
        public function __get(string $name)
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
            'url' => Helper\Url::class,
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

    class Url
    {
        public function __invoke(?string $name = null, array $params = [], array $options = []): string
        {
            return '/';
        }
    }
}
