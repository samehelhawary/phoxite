<?php

namespace Joomla\CMS\Event;

// A stand-in: the map of event names to event classes.
trait CoreEventAware
{
    private static $eventNameToConcreteClass = [
        // Content
        'onContentPrepare'   => Content\ContentPrepareEvent::class,
        'onContentAfterSave' => Content\AfterSaveEvent::class,
    ];
}
