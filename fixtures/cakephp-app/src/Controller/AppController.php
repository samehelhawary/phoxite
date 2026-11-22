<?php

namespace App\Controller;

use Cake\Controller\Controller;

class AppController extends Controller
{
    public function initialize(): void
    {
        $this->loadComponent('Flash');
        $this->loadComponent('Shelf');
    }

    public function beforeRender(): void
    {
        $this->set('library', 'City Library');
    }
}
