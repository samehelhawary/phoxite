<?php

namespace App\Controller;

class AuthorsController extends AppController
{
    public function view(int $id): void
    {
        $author = $this->Authors->get($id);
        $this->set('author', $author);
    }
}
