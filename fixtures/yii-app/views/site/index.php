<?php

use app\widgets\Shelf;
use yii\helpers\Html;

$this->title = $heading;

?>
<h1><?= Html::encode($heading) ?></h1>
<?= Shelf::widget(['books' => $books]) ?>
<?php foreach ($books as $book): ?>
    <?= $this->render('//book/_row', ['book' => $book]) ?>
<?php endforeach; ?>
