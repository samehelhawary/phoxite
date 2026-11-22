<ul>
<?php foreach ($this->items as $item) : ?>
    <li><?php echo $this->escape($item->title); ?></li>
<?php endforeach; ?>
</ul>
