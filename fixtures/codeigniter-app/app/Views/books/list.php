<?php $this->extend('layout') ?>
<?php $this->section('content') ?>
<h1><?= esc($heading) ?></h1>
<p><?= $catalog->count() ?> books, in <?= esc($currency) ?></p>
<ul>
<?php foreach ($titles as $title): ?>
    <li><?= esc($title) ?></li>
<?php endforeach ?>
</ul>
<?php $this->endSection() ?>
