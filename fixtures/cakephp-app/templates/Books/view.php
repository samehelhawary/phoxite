<h1><?= $book->label() ?></h1>
<p><?= $this->Stars->show(count($reviews)) ?></p>
<?= $this->Html->link('The author', ['controller' => 'Authors', 'action' => 'view', $book->author_id]) ?>
