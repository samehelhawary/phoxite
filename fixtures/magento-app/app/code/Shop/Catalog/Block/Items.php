<?php

namespace Shop\Catalog\Block;

use Magento\Framework\View\Element\Template;

class Items extends Template
{
    /**
     * @return string[]
     */
    public function names(): array
    {
        return ['Tea', 'Milk'];
    }
}
