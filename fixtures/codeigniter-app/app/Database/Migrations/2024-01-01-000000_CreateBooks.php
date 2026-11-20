<?php

namespace App\Database\Migrations;

use CodeIgniter\Database\Migration;

class CreateBooks extends Migration
{
    public function up(): void
    {
        $this->forge->addField([
            'id' => ['type' => 'INT'],
            'title' => ['type' => 'VARCHAR', 'constraint' => 200],
            'price' => ['type' => 'INT'],
        ]);
        $this->forge->createTable('books');
    }

    public function down(): void
    {
        $this->forge->dropTable('books');
    }
}
