<?php

namespace App\Controllers;

use CodeIgniter\Controller;
use CodeIgniter\Database\BaseConnection;
use CodeIgniter\Events\Events;
use CodeIgniter\Validation\Validation;

class Orders extends Controller
{
    public function index(BaseConnection $db, Validation $validation): string
    {
        $validation->setRules(['email' => 'required|valid_email', 'title' => 'required|min_length[3]']);
        if (! $validation->run(['email' => 'a@b.c'], 'order')) {
            return route_to('orders');
        }
        Events::trigger('order_placed', 1);
        $count = $db->table('books')->where('title', 'Dune')->orderBy('price')->countAllResults();

        return view_cell('Card::render') . $count;
    }
}
