<?php

namespace Acme\Component\Shop\Site\Controller;

use Joomla\CMS\MVC\Controller\BaseController;

class DisplayController extends BaseController
{
    public function count(): int
    {
        return $this->getModel('Items')->total();
    }
}
