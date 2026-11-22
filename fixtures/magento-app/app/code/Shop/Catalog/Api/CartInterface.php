<?php

namespace Shop\Catalog\Api;

interface CartInterface
{
    public function total(): float;

    public function save(int $times = 1): bool;
}
