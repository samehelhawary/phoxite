<?php

use yii\helpers\Html;

/** @var string $content */

?>
<!DOCTYPE html>
<html>
<head>
    <title><?= Html::encode($this->title) ?></title>
</head>
<body>
<nav>
    <?= Html::a('Home', ['site/index']) ?>
    <?= Html::a('Contact', ['site/contact']) ?>
</nav>
<?= $content ?>
</body>
</html>
