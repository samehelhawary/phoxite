<?php

namespace Drupal\shop;

use Drupal\shop\Entity\Book;

class Prices
{
    public function of(Book $book): int
    {
        return strlen($book->title);
    }
}
