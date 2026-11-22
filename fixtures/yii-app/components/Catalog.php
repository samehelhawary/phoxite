<?php

namespace app\components;

use app\models\Book;
use yii\base\Component;

class Catalog extends Component
{
    /** @var int */
    public $perPage = 10;

    /**
     * @return Book[]
     */
    public function newest(): array
    {
        return Book::find()->orderBy('id')->all();
    }

    public function getSize(): int
    {
        return count($this->newest());
    }
}
