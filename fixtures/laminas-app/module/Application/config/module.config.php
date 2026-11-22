<?php

namespace Application;

use Laminas\ServiceManager\Factory\InvokableFactory;

return [
    'router' => [
        'routes' => [
            'home' => [
                'type' => 'Literal',
                'options' => [
                    'route' => '/',
                    'defaults' => [
                        'controller' => Controller\BookController::class,
                        'action' => 'index',
                    ],
                ],
            ],
            'book' => [
                'type' => 'Literal',
                'options' => [
                    'route' => '/book',
                    'defaults' => [
                        'controller' => Controller\BookController::class,
                        'action' => 'index',
                    ],
                ],
                'child_routes' => [
                    'view' => [
                        'type' => 'Segment',
                        'options' => [
                            'route' => '/:id',
                            'defaults' => [
                                'action' => 'view',
                            ],
                        ],
                    ],
                ],
            ],
        ],
    ],
    'controllers' => [
        'factories' => [
            Controller\BookController::class => Controller\BookControllerFactory::class,
        ],
    ],
    'service_manager' => [
        'aliases' => [
            'catalog' => Service\Catalog::class,
        ],
        'factories' => [
            Service\Catalog::class => Service\CatalogFactory::class,
            Service\Clock::class => InvokableFactory::class,
        ],
        'invokables' => [
            'clock' => Service\Clock::class,
        ],
    ],
    'controller_plugins' => [
        'invokables' => [
            'shelf' => Controller\Plugin\Shelf::class,
        ],
    ],
    'view_helpers' => [
        'invokables' => [
            'stars' => View\Helper\Stars::class,
        ],
    ],
    'view_manager' => [
        'not_found_template' => 'error/404',
        'template_map' => [
            'layout/layout' => __DIR__ . '/../view/layout/layout.phtml',
            'error/404' => __DIR__ . '/../view/error/404.phtml',
        ],
        'template_path_stack' => [
            __DIR__ . '/../view',
        ],
    ],
];
