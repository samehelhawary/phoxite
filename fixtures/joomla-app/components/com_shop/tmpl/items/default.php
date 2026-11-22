<?php

use Joomla\CMS\Language\Text;

?>
<h1><?php echo $this->escape($this->heading()); ?></h1>
<p><?php echo Text::sprintf('COM_SHOP_HITS', count($this->items)); ?></p>
<?php echo $this->loadTemplate('row'); ?>
