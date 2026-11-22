<?php

namespace Shop\Catalog\Model\ResourceModel;

use Magento\Framework\Model\ResourceModel\Db\AbstractDb;

class Cart extends AbstractDb
{
    protected function _construct()
    {
        $this->_init('shop_cart', 'cart_id');
    }
}
