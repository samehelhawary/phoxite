<?php

use yii\helpers\Html;
use yii\helpers\Url;

?>
<form action="<?= Url::to(['contact']) ?>" method="post">
    <label><?= Html::encode($form->getAttributeLabel('email')) ?></label>
    <input name="email" value="<?= Html::encode($form->email) ?>">
    <label><?= Html::encode($form->getAttributeLabel('body')) ?></label>
    <textarea name="body"><?= Html::encode($form->body) ?></textarea>
</form>
