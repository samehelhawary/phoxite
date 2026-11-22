<?php

namespace App\View\Helper;

use Cake\View\Helper;

class StarsHelper extends Helper
{
    public function show(int $rating): string
    {
        return str_repeat('*', $rating);
    }
}
