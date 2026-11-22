<?php

namespace Acme\Component\Shop\Administrator\Extension;

use Joomla\CMS\Extension\MVCComponent;

class ShopComponent extends MVCComponent
{
    public function currency(): string
    {
        return 'EUR';
    }
}
