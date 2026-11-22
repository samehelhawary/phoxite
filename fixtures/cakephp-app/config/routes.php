<?php

use Cake\Routing\RouteBuilder;

return function (RouteBuilder $routes): void {
    $routes->scope('/', function (RouteBuilder $builder): void {
        $builder->connect('/', ['controller' => 'Books', 'action' => 'index'], ['_name' => 'home']);
        $builder->connect('/books/{id}', 'Books::view');
        $builder->connect('/authors/{id}', ['controller' => 'Authors', 'action' => 'view']);
    });
};
