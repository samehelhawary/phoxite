<h1><?= $heading ?> (<?= $library ?>, <?= $capacity ?>)</h1>
<?php foreach ($books as $book): ?>
    <?= $this->element('book_row', ['book' => $book]) ?>
<?php endforeach; ?>
<?= $this->Html->link('Add a book', ['action' => 'add']) ?>
<?= $this->cell('Newest') ?>
