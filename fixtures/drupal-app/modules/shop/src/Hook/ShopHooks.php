<?php

namespace Drupal\shop\Hook;

use Drupal\Core\Hook\Attribute\Hook;

class ShopHooks
{
    #[Hook('shop_book_presave')]
    public function saved($entity): string
    {
        return $entity->label();
    }

    #[Hook('cron')]
    public function cron(): void
    {
        if (\Drupal::moduleHandler()->moduleExists('shop')) {
            \Drupal::moduleHandler()->invokeAll('entity_presave', []);
        }
    }
}
