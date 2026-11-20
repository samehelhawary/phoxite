<?php

namespace Drupal\shop\Controller;

use Drupal\Core\Access\AccessResult;
use Drupal\Core\Block\BlockManagerInterface;
use Drupal\Core\Controller\ControllerBase;
use Drupal\Core\Link;
use Drupal\Core\Session\AccountInterface;
use Drupal\Core\Url;

class BookController extends ControllerBase
{
    public function __construct(private AccountInterface $account, private BlockManagerInterface $blocks)
    {
    }

    /** @return array<string, mixed> */
    public function list(): array
    {
        $catalog = \Drupal::service('shop.catalog');
        $build = ['#theme' => 'shop_book', '#books' => $catalog->books()];
        $build['#attached']['library'][] = 'shop/base';
        if ($this->account->hasPermission('administer shop')) {
            $build['#link'] = Link::createFromRoute('Books', 'shop.books');
        }
        $build['#cart'] = $this->blocks->createInstance('shop_cart');
        $build['#currency'] = $this->config('shop.settings')->get('currency');
        return $build;
    }

    public function show(): string
    {
        if (!AccessResult::allowedIfHasPermission($this->account, 'access shop')) {
            return $this->redirect('shop.books');
        }
        $shelf = \Drupal::entityTypeManager()->getStorage('shop_shelf')->load(1);
        $room = $shelf === null ? '' : $shelf->room;
        return Url::fromRoute('shop.book', ['book' => 1])->toString() . $room;
    }
}
