<?php

namespace Acme\Plugin\Content\Stamp\Extension;

use Joomla\CMS\Language\Text;
use Joomla\CMS\Plugin\CMSPlugin;
use Joomla\Event\Event;

final class Stamp extends CMSPlugin
{
    /** @var string[] */
    private array $log = [];

    public static function getSubscribedEvents(): array
    {
        return [
            'onContentPrepare'   => 'stamp',
            'onContentAfterSave' => 'onContentAfterSave',
        ];
    }

    public function stamp($event): void
    {
        $item = $event->getItem();
        $this->log[] = $item::class;
    }

    public function onContentAfterSave(Event $event): void
    {
        if ($event->getIsNew() && $this->getApplication()->getIdentity()) {
            $this->log[] = Text::_('PLG_CONTENT_STAMP_SAVED');
        }
    }

    public function count(): int
    {
        return count($this->log);
    }
}
