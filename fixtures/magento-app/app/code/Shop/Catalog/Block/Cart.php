<?php

namespace Shop\Catalog\Block;

use Magento\Framework\View\Element\Template;

class Cart extends Template
{
    protected $_template = 'Shop_Catalog::cart.phtml';

    public function title(): string
    {
        return 'Your cart';
    }

    public function compact(): static
    {
        $this->setTemplate('Shop_Catalog::cart/summary.phtml');
        return $this;
    }
}
