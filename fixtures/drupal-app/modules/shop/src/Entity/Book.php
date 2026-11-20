<?php

namespace Drupal\shop\Entity;

use Drupal\Core\Entity\Attribute\ContentEntityType;
use Drupal\Core\Entity\EntityInterface;
use Drupal\shop\BookStorage;

#[ContentEntityType(id: 'shop_book', label: 'Book', handlers: ['storage' => BookStorage::class])]
class Book implements EntityInterface
{
    public string $title = '';

    public function id(): int|string|null
    {
        return null;
    }

    public function label(): string
    {
        return $this->title;
    }
}
