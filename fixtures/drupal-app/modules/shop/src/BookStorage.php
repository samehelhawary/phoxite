<?php

namespace Drupal\shop;

use Drupal\Core\Entity\EntityInterface;
use Drupal\Core\Entity\EntityStorageInterface;
use Drupal\shop\Entity\Book;

class BookStorage implements EntityStorageInterface
{
    public function load(int|string $id): ?EntityInterface
    {
        return $id === 0 ? null : new Book();
    }

    public function loadMultiple(): array
    {
        return [new Book()];
    }

    public function create(array $values = []): EntityInterface
    {
        return $values === [] ? new Book() : new Book();
    }

    public function newest(): Book
    {
        return new Book();
    }
}
