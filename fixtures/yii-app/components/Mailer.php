<?php

namespace app\components;

use yii\mail\BaseMailer;
use yii\mail\MessageInterface;

class Mailer extends BaseMailer
{
    public function compose($view = null, array $params = []): MessageInterface
    {
        return new Message((string) $view, $params);
    }
}
