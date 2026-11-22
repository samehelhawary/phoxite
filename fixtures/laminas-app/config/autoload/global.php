<?php

return [
    'library' => [
        'name' => 'City Library',
        'per_page' => 20,
    ],
    'service_manager' => [
        'factories' => [
            'year' => function (): \Application\Service\Clock {
                return new \Application\Service\Clock();
            },
        ],
    ],
];
