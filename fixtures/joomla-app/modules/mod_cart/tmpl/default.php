<?php

?>
<div class="cart" data-limit="<?php echo (int) $params->get('limit'); ?>">
    <?php echo $app->getIdentity() ? 'Yours' : 'A guest\'s'; ?>
</div>
