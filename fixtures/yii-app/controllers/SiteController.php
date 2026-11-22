<?php

namespace app\controllers;

use app\models\ContactForm;
use Yii;
use yii\web\Controller;

class SiteController extends Controller
{
    public $layout = 'main';

    public function actionIndex()
    {
        $catalog = Yii::$app->catalog;

        return $this->render('index', [
            'books' => $catalog->newest(),
            'heading' => Yii::t('app', 'Newest books'),
        ]);
    }

    public function actionContact()
    {
        $form = new ContactForm();
        if ($form->load(Yii::$app->request->post()) && $form->send()) {
            return $this->redirect(['book/view', 'id' => 1]);
        }

        return $this->render('contact', ['form' => $form]);
    }

    public function actionFlush()
    {
        Yii::$app->get('cache')->flush();

        return $this->redirect(['index']);
    }
}
