<?php

namespace App\Model\Table;

use Cake\ORM\Table;

class BooksTable extends Table
{
    public function initialize(array $config): void
    {
        $this->setTable('books');
        $this->addBehavior('Timestamp');
        $this->belongsTo('Authors');
        $this->hasMany('Reviews', ['className' => 'Reviews.Reviews']);
    }

    public function countPublished(): int
    {
        return count($this->find()->where(['published' => true])->toArray());
    }
}
