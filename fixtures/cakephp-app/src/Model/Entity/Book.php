<?php

namespace App\Model\Entity;

use Cake\ORM\Entity;

/**
 * @property int $id
 * @property string $title
 * @property int $author_id
 */
class Book extends Entity
{
    /** A column of the table the migration makes, which the class doesn't document. */
    public function isLong(): bool
    {
        return (int) $this->pages > 300;
    }

    public function label(): string
    {
        $author = $this->author;

        return $author === null ? $this->title : $this->title . ' (' . $author->name . ')';
    }
}
