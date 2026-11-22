<?php

namespace Application\Model;

class Book
{
    public function __construct(public int $id, public string $title, public int $rating)
    {
    }
}
