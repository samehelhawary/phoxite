<?php

namespace Shop\Catalog\Controller\Cart;

use Magento\Framework\App\Action\Action;

class Index extends Action
{
    public function execute()
    {
        $this->_redirect('shop/cart/index');
        return null;
    }
}
