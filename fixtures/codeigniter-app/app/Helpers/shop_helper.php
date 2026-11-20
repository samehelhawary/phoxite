<?php

function shop_price(int $cents): string
{
    return number_format($cents / 100, 2);
}
