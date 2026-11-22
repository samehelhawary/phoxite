<?php

return [
    'modules' => require __DIR__ . '/modules.config.php',
    'module_listener_options' => [
        'config_glob_paths' => [
            realpath(__DIR__) . '/autoload/{{,*.}global,{,*.}local}.php',
        ],
    ],
];
