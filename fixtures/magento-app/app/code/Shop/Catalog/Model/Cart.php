<?php

namespace Shop\Catalog\Model;

use Magento\Framework\DataObject;
use Shop\Catalog\Api\CartInterface;

class Cart extends DataObject implements CartInterface
{
    public function total(): float
    {
        return (float) $this->getData('total');
    }

    public function save(int $times = 1): bool
    {
        return $times > 0;
    }
}
