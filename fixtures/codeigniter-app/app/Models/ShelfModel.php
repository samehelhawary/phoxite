<?php

namespace App\Models;

use CodeIgniter\Model;

class ShelfModel extends Model
{
    protected $table = 'books';

    /** @var list<string> */
    protected $allowedFields = ['title', 'price'];

    /** @return list<string> */
    public function fields(): array
    {
        return $this->allowedFields;
    }
}
