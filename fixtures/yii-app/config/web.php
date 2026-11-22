<?php

$params = require __DIR__ . '/params.php';
$db = require __DIR__ . '/db.php';

$config = [
    'id' => 'library',
    'basePath' => dirname(__DIR__),
    'defaultRoute' => 'site/index',
    'aliases' => [
        '@covers' => '@app/web/covers',
    ],
    'components' => [
        'cache' => [
            'class' => 'yii\caching\FileCache',
        ],
        'catalog' => [
            'class' => \app\components\Catalog::class,
            'perPage' => 20,
        ],
        'mailer' => [
            'class' => 'app\components\Mailer',
        ],
        'user' => [
            'identityClass' => 'app\models\Author',
            'loginUrl' => ['site/contact'],
        ],
        'db' => $db,
    ],
    'modules' => [
        'admin' => [
            'class' => 'app\modules\admin\Module',
        ],
    ],
    'params' => $params,
];

if (YII_ENV_DEV) {
    $config['bootstrap'][] = 'admin';
}

return $config;
