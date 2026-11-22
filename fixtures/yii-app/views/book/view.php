<?php

use yii\helpers\Html;

?>
<h1><?= Html::encode($book->label) ?></h1>
<img src="<?= Html::encode($covers) ?>/<?= $book->id ?>.png" alt="">
<?= $this->render('_row', ['book' => $book]) ?>
