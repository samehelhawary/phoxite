<?php

namespace Application\View\Helper;

class Stars
{
    public function __invoke(int $rating): string
    {
        return str_repeat('*', $rating);
    }
}
