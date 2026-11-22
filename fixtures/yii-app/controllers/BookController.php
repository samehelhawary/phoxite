<?php

namespace app\controllers;

use app\models\Book;
use Yii;
use yii\helpers\Url;
use yii\web\Controller;

class BookController extends Controller
{
    public function actionView($id)
    {
        $book = Book::find()->where(['id' => $id])->one();
        if ($book === null) {
            return $this->redirect(Url::to(['site/index']));
        }

        return $this->render('view', [
            'book' => $book,
            'covers' => Yii::getAlias('@covers'),
        ]);
    }

    public function actionByAuthor($id)
    {
        return $this->redirect(['/admin/report/index', 'author' => $id]);
    }
}
