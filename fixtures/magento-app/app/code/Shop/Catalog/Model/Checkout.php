<?php

namespace Shop\Catalog\Model;

use Magento\Framework\App\Config\ScopeConfigInterface;
use Magento\Framework\AuthorizationInterface;
use Magento\Framework\Event\ManagerInterface;
use Magento\Framework\Module\Manager;
use Magento\Framework\ObjectManagerInterface;
use Magento\Framework\View\LayoutInterface;
use Shop\Catalog\Api\Data\ItemInterface;

class Checkout
{
    public function __construct(
        private ManagerInterface $events,
        private ScopeConfigInterface $config,
        private AuthorizationInterface $authorization,
        private Manager $modules,
        private ObjectManagerInterface $objects,
        private LayoutInterface $layout,
        private CartFactory $cartFactory,
        private Cart\Proxy $lazyCart,
    ) {
    }

    public function place(ItemInterface $item): bool
    {
        if (!$this->modules->isEnabled('Shop_Catalog') || !$this->authorization->isAllowed('Shop_Catalog::cart_edit')) {
            return false;
        }
        $limit = (int) $this->config->getValue('shop/cart/limit');
        $cart = $this->cartFactory->create();
        $cart->setName($item->getName())->setLimit($limit);
        $note = $item->getExtensionAttributes()?->getGiftNote();
        $logger = $this->objects->get('ShopLogger');
        $logger->log($note ?? 'no note');
        $this->objects->get(Logger::class)->log('placed');
        $saved = $cart->save() && $this->lazyCart->total() >= 0;
        $this->events->dispatch('shop_cart_saved', ['cart' => $cart]);
        return $saved;
    }

    public function render(): string
    {
        $block = $this->layout->getBlock('shop.cart');
        return $block ? 'cart' : '';
    }
}
