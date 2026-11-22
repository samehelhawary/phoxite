<?php

namespace Acme\Component\Shop\Site\View\Items;

use Joomla\CMS\MVC\View\HtmlView as BaseHtmlView;

class HtmlView extends BaseHtmlView
{
    /** @var \Acme\Component\Shop\Site\Model\Item[] */
    public $items = [];

    public function display($tpl = null)
    {
        $this->items = $this->get('Items');
        parent::display($tpl);
    }

    public function heading(): string
    {
        return 'Shop';
    }
}
