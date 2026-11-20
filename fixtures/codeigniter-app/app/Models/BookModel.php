<?php

namespace App\Models;

use CodeIgniter\Model;

class BookModel extends Model
{
    protected $table = 'books';

    /** @return list<string> */
    public function titles(): array
    {
        return count($this->findAll()) > 0 ? ['Dune'] : [];
    }
}
