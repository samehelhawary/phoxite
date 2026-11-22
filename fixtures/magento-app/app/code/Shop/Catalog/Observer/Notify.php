<?php

namespace Shop\Catalog\Observer;

use Magento\Framework\Event\Observer;
use Magento\Framework\Event\ObserverInterface;
use Shop\Catalog\Model\Logger;

class Notify implements ObserverInterface
{
    public function __construct(private Logger $logger)
    {
    }

    public function execute(Observer $observer)
    {
        $this->logger->log('cart ' . ($observer->hasCart() ? 'saved' : 'emptied'));
    }
}
