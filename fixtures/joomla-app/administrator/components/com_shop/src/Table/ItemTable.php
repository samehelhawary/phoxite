<?php

namespace Acme\Component\Shop\Administrator\Table;

use Joomla\CMS\Table\Table;

class ItemTable extends Table
{
    public function __construct($db)
    {
        parent::__construct('#__shop_items', 'id', $db);
    }

    public function stock(): int
    {
        return 0;
    }
}
