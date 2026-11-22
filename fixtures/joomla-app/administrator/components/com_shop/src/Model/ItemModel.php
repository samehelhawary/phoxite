<?php

namespace Acme\Component\Shop\Administrator\Model;

use Joomla\CMS\MVC\Model\FormModel;

class ItemModel extends FormModel
{
    public function getForm($data = [], $loadData = true)
    {
        return $this->loadForm('com_shop.item', 'item', ['control' => 'jform', 'load_data' => $loadData, 'data' => $data]);
    }

    public function publish(int $id): bool
    {
        return $id > 0;
    }
}
