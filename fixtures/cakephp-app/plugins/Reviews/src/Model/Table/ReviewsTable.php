<?php

namespace Reviews\Model\Table;

use Cake\ORM\Table;

class ReviewsTable extends Table
{
    public function initialize(array $config): void
    {
        $this->belongsTo('Books');
    }
}
