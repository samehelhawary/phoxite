<?php

namespace app\models;

use Yii;
use yii\db\ActiveRecord;

/**
 * @property int $id
 * @property string $title
 * @property int $author_id
 */
class Book extends ActiveRecord
{
    public function rules()
    {
        return [
            [['title', 'author_id'], 'required'],
            ['title', 'string', 'max' => 200],
        ];
    }

    public function attributeLabels()
    {
        return [
            'title' => Yii::t('app', 'Title'),
            'author_id' => Yii::t('app', 'Author'),
        ];
    }

    public function getAuthor()
    {
        return $this->hasOne(Author::class, ['id' => 'author_id']);
    }

    /** A column of the table the migration makes, which the class doesn't document. */
    public function isLong(): bool
    {
        return (int) $this->pages > 300;
    }

    public function getLabel(): string
    {
        $author = $this->author;

        return $author === null ? $this->title : $this->title . ' (' . $author->name . ')';
    }
}
