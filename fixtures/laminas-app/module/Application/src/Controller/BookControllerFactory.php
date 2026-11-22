<?php

namespace Application\Controller;

use Psr\Container\ContainerInterface;

class BookControllerFactory
{
    public function __invoke(ContainerInterface $container): BookController
    {
        return new BookController($container->get('catalog'));
    }
}
