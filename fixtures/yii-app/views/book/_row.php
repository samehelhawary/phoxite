<?php

use yii\helpers\Html;

?>
<p><?= Html::a(Html::encode($book->title), ['book/view', 'id' => $book->id]) ?></p>
