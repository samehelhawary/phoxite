<?php

namespace Drupal\shop\Entity;

use Drupal\Core\Entity\EntityInterface;

/**
 * A shelf of books.
 *
 * @ContentEntityType(
 *   id = "shop_shelf",
 *   label = @Translation("Shelf"),
 * )
 */
class Shelf implements EntityInterface
{
    public string $room = '';

    public function id(): int|string|null
    {
        return null;
    }

    public function label(): string
    {
        return $this->room;
    }
}
