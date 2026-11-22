<?php

namespace Shop\Catalog\Model;

class Logger
{
    /** @var string[] */
    private array $lines = [];

    public function log(string $line): void
    {
        $this->lines[] = $line;
    }

    public function count(): int
    {
        return count($this->lines);
    }
}
