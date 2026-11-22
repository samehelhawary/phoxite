<?php

use yii\helpers\Html;

?>
<ul>
<?php foreach ($authors as $author): ?>
    <li><?= Html::encode($author->name) ?> (<?= count($author->books) ?>)</li>
<?php endforeach; ?>
</ul>
<?= Html::a('Back', ['/site/index']) ?>
