<?php

namespace Shop\Catalog\Api\Data;

interface ItemInterface
{
    public function getName(): string;

    /**
     * @return \Shop\Catalog\Api\Data\ItemExtensionInterface|null
     */
    public function getExtensionAttributes();
}
