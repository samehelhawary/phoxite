<?php

namespace Config;

use CodeIgniter\Validation\FormatRules;
use CodeIgniter\Validation\Rules;

class Validation
{
    /** @var list<class-string> */
    public array $ruleSets = [Rules::class, FormatRules::class];

    /** @var array<string, string> */
    public array $order = ['email' => 'required|valid_email'];
}
