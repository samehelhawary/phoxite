<?php

use CodeIgniter\Router\RouteCollection;

/** @var RouteCollection $routes */
$routes->get('books', 'Books::index', ['as' => 'books.list', 'filter' => 'auth']);
$routes->get('books/(:num)', 'Books::show/$1', ['as' => 'books.show']);
$routes->get('orders', 'Orders::index');
