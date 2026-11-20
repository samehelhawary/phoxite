<?php

namespace App\Controllers;

use CodeIgniter\Controller;
use CodeIgniter\HTTP\RedirectResponse;

class Books extends Controller
{
    protected $helpers = ['shop'];

    public function index(): string
    {
        helper(['shop', 'url']);
        $model = model('BookModel');
        $catalog = service('catalog');
        $shop = config('Shop');
        return view('books/list', [
            'titles' => $model->titles(),
            'catalog' => $catalog,
            'currency' => $shop->currency,
            'heading' => lang('Shop.cart.empty'),
        ]);
    }

    public function show(): RedirectResponse
    {
        return redirect()->route('books.list');
    }

    public function link(): string
    {
        return route_to('books.show', 1) . site_url('books') . shop_price(100);
    }
}
