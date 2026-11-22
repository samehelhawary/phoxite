<?php

namespace App\View\Cell;

use Cake\View\Cell;

class NewestCell extends Cell
{
    public function display(): void
    {
        $newest = $this->fetchTable('Books')->find()->first();
        $this->set('newest', $newest);
    }
}
