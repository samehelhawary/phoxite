<?php

namespace Application\Service;

use Psr\Container\ContainerInterface;

class CatalogFactory
{
    public function __invoke(ContainerInterface $container): Catalog
    {
        $config = $container->get('config');
        $name = $config['library']['name'];

        return new Catalog((string) $name, $container->get('clock'));
    }
}
