<?php

namespace app\modules\admin\controllers;

use app\models\Author;
use yii\web\Controller;

class ReportController extends Controller
{
    public function actionIndex()
    {
        return $this->render('index', ['authors' => Author::find()->all()]);
    }
}
