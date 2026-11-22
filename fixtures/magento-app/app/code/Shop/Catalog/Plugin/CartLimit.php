<?php

namespace Shop\Catalog\Plugin;

use Shop\Catalog\Model\Cart;

class CartLimit
{
    public function beforeSave(Cart $subject, int $times = 1): array
    {
        return [min($times, $this->limit($subject))];
    }

    public function afterTotal(Cart $subject, float $result): float
    {
        return min($result, 1000.0 * $this->limit($subject));
    }

    private function limit(Cart $cart): int
    {
        return $cart->hasLimit() ? 5 : 10;
    }
}
