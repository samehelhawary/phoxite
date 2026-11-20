<?php

namespace Drupal\shop;

use Drupal\Core\Entity\EntityTypeManagerInterface;
use Drupal\shop\Entity\Book;

class Catalog
{
    public function __construct(private Prices $prices, private EntityTypeManagerInterface $entities)
    {
    }

    /** @return list<Book> */
    public function books(): array
    {
        $storage = $this->entities->getStorage('shop_book');
        $first = $storage->load(1);
        $found = $first === null ? [] : [$first];
        foreach ($storage->loadMultiple() as $book) {
            if ($this->prices->of($book) > 0 && $book->title !== '') {
                $found[] = $book;
            }
        }
        return $found;
    }
}
