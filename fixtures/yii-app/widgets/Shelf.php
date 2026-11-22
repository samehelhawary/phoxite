<?php

namespace app\widgets;

use app\models\Book;
use yii\base\Widget;

class Shelf extends Widget
{
    /** @var Book[] */
    public $books = [];

    public function run()
    {
        return $this->render('shelf', ['count' => count($this->books)]);
    }
}
