<?php

namespace App\Controller\Component;

use Cake\Controller\Component;

class ShelfComponent extends Component
{
    public function capacity(): int
    {
        return 40;
    }
}
