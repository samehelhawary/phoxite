<h1><?= $author->name ?> (<?= $author->bookCount() ?>)</h1>
<?php foreach ($author->books as $book): ?>
    <?= $this->element('book_row', ['book' => $book]) ?>
<?php endforeach; ?>
