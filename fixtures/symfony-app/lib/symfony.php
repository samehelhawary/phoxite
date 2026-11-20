<?php

/*
 * A stand-in for Symfony's API, written for this fixture: the names of its classes and
 * methods only.
 */

namespace Psr\Container {
    interface ContainerInterface
    {
        /** @return mixed */
        public function get(string $id);

        public function has(string $id): bool;
    }
}

namespace Symfony\Bundle\FrameworkBundle {
    class FrameworkBundle
    {
    }
}

namespace Symfony\Contracts\Translation {
    interface TranslatorInterface
    {
        /** @param array<string, string> $parameters */
        public function trans(string $id, array $parameters = []): string;
    }
}

namespace Symfony\Bundle\FrameworkBundle\Controller {
    abstract class AbstractController
    {
        /** @param array<string, mixed> $parameters */
        protected function render(string $view, array $parameters = []): string
        {
            return $view . count($parameters);
        }

        /** @param array<string, mixed> $parameters */
        protected function generateUrl(string $route, array $parameters = []): string
        {
            return $route . count($parameters);
        }

        /** @param array<string, mixed> $parameters */
        protected function redirectToRoute(string $route, array $parameters = []): string
        {
            return $route . count($parameters);
        }
    }
}
