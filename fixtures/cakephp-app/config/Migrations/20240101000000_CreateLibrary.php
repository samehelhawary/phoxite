<?php

use Migrations\AbstractMigration;

class CreateLibrary extends AbstractMigration
{
    public function change(): void
    {
        $this->table('authors')
            ->addColumn('name', 'string', ['limit' => 120, 'null' => false])
            ->create();

        $table = $this->table('books');
        $table->addColumn('title', 'string', ['limit' => 200, 'null' => false]);
        $table->addColumn('author_id', 'integer', ['null' => false]);
        $table->addColumn('pages', 'integer', ['null' => true]);
        $table->create();
    }
}
