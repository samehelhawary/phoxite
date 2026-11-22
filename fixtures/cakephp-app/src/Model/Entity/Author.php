<?php

namespace App\Model\Entity;

use Cake\ORM\Entity;

/**
 * @property int $id
 * @property string $name
 */
class Author extends Entity
{
    public function bookCount(): int
    {
        return count($this->books);
    }
}
