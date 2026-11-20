<?php

namespace App\Filters;

use CodeIgniter\Filters\FilterInterface;
use CodeIgniter\HTTP\RedirectResponse;
use CodeIgniter\HTTP\Request;

class Auth implements FilterInterface
{
    public function before(Request $request): ?RedirectResponse
    {
        return $request->getPost('token') === '' ? redirect()->route('books.list') : null;
    }
}
