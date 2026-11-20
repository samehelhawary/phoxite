<?php

namespace Config;

use App\Libraries\Catalog;
use CodeIgniter\Config\BaseService;

class Services extends BaseService
{
    public static function catalog(): Catalog
    {
        return new Catalog();
    }
}
