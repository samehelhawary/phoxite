<?php foreach ($reviews as $review): ?>
    <p><?= $review->rating ?>: <?= $review->body ?></p>
<?php endforeach; ?>
