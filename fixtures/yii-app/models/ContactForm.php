<?php

namespace app\models;

use Yii;
use yii\base\Model;

class ContactForm extends Model
{
    /** @var string */
    public $email = '';

    /** @var string */
    public $body = '';

    public function rules()
    {
        return [
            [['email', 'body'], 'required'],
            ['email', 'email'],
        ];
    }

    public function attributeLabels()
    {
        return [
            'email' => Yii::t('app', 'Your email'),
            'body' => Yii::t('app', 'Message'),
        ];
    }

    public function send(): bool
    {
        if (!$this->validate()) {
            $this->addError('body', Yii::t('app', 'Message'));

            return false;
        }

        $message = Yii::$app->mailer->compose('welcome', ['form' => $this]);

        $message->setTo(Yii::$app->params['mail']['from']);

        return $message->setTo(Yii::$app->params['adminEmail'])->setTo($this->email)->send();
    }
}
