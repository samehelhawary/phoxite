<?php

namespace Application\Service;

use Application\Model\Book;

class Catalog
{
    public function __construct(private string $name, private Clock $clock)
    {
    }

    public function title(): string
    {
        return $this->name . ' ' . $this->clock->year();
    }

    /** @return list<Book> */
    public function newest(): array
    {
        return [new Book(1, 'Middlemarch', 5)];
    }

    public function find(int $id): ?Book
    {
        return $id === 1 ? new Book(1, 'Middlemarch', 5) : null;
    }
}
