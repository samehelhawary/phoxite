<?php

namespace Drupal\shop\Plugin\Block;

use Drupal\Core\Block\Attribute\Block;
use Drupal\Core\Block\BlockBase;

#[Block(id: 'shop_cart', admin_label: 'Cart')]
class CartBlock extends BlockBase
{
    public function build(): array
    {
        return ['#theme' => 'shop_book', '#attached' => ['library' => ['shop/cart']]];
    }
}
