<?php

namespace Config;

use App\Filters\Auth;

class Filters
{
    /** @var array<string, class-string> */
    public array $aliases = [
        'auth' => Auth::class,
    ];

    /** @var array<string, list<string>> */
    public array $globals = [
        'before' => ['auth'],
        'after' => [],
    ];
}
