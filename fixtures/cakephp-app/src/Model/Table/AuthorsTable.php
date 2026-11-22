<?php

namespace App\Model\Table;

use Cake\ORM\Table;

class AuthorsTable extends Table
{
    public function initialize(array $config): void
    {
        $this->hasMany('Books');
    }
}
