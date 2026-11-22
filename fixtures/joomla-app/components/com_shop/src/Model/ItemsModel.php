<?php

namespace Acme\Component\Shop\Site\Model;

use Joomla\CMS\MVC\Model\BaseDatabaseModel;

class ItemsModel extends BaseDatabaseModel
{
    /**
     * @return Item[]
     */
    public function getItems(): array
    {
        return [new Item()];
    }

    public function total(): int
    {
        return count($this->getItems());
    }
}
