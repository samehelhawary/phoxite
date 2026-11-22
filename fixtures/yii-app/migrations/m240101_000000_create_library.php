<?php

use yii\db\Migration;

class m240101_000000_create_library extends Migration
{
    public function safeUp()
    {
        $this->createTable('{{%author}}', [
            'id' => $this->primaryKey(),
            'name' => $this->string(120)->notNull(),
        ]);
        $this->createTable('{{%book}}', [
            'id' => $this->primaryKey(),
            'title' => $this->string(200)->notNull(),
            'author_id' => $this->integer()->notNull(),
            'pages' => $this->integer(),
        ]);
        $this->addColumn('{{%book}}', 'published_at', $this->string(10));
    }

    public function safeDown()
    {
        $this->dropTable('{{%book}}');
        $this->dropTable('{{%author}}');
    }
}
