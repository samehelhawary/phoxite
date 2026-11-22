<?php

namespace app\components;

use yii\mail\MessageInterface;

class Message implements MessageInterface
{
    /** @var string[] */
    private array $to = [];

    public function __construct(private string $view, private array $params)
    {
    }

    public function setTo($to)
    {
        $this->to[] = (string) $to;

        return $this;
    }

    public function send()
    {
        return $this->view !== '' && $this->to !== [] && $this->params !== [];
    }
}
