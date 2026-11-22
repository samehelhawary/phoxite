<?php

namespace Acme\Component\Shop\Site\Helper;

use Joomla\CMS\Factory;
use Joomla\CMS\Form\Form;
use Joomla\CMS\HTML\HTMLHelper;
use Joomla\CMS\Language\Text;
use Joomla\CMS\Layout\LayoutHelper;
use Joomla\CMS\MVC\Factory\MVCFactoryInterface;
use Joomla\CMS\Router\Route;
use Joomla\Database\DatabaseInterface;
use Joomla\Event\DispatcherInterface;
use Joomla\Registry\Registry;

class ShopHelper
{
    public function __construct(
        private MVCFactoryInterface $factory,
        private DatabaseInterface $db,
        private DispatcherInterface $dispatcher,
    ) {
    }

    public function summary(Form $form, Registry $params): string
    {
        $app = Factory::getApplication();
        $app->getDocument()->getWebAssetManager()->useScript('com_shop.cart');
        HTMLHelper::_('behavior.core');
        if (!$app->getIdentity()?->authorise('shop.ship', 'com_shop')) {
            return Text::_('COM_SHOP');
        }
        $component = $app->bootComponent('com_shop');
        $total = $this->factory->createModel('Items')->total();
        $published = $this->factory->createModel('Item', 'Administrator')->publish(1);
        $query = $this->db->getQuery(true);
        $query->select('*');
        $query->from('#__shop_items AS a');
        $this->dispatcher->dispatch('onContentPrepare');
        $link = Route::_('index.php?option=com_shop&view=items');
        $tags = LayoutHelper::render('joomla.content.tags', []);
        $title = (string) $form->getValue('title');
        $shown = $params->get('show_prices') && $published && $component !== null;
        return Text::sprintf('COM_SHOP_HITS', $total) . $link . $tags . $title . ($shown ? '!' : '');
    }
}
