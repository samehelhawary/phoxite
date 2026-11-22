<?php

namespace App\Controller;

use Cake\Core\Configure;
use Cake\Routing\Router;

class BooksController extends AppController
{
    public function index(): void
    {
        $books = $this->Books->find()->where(['published' => true])->toArray();
        $heading = Configure::read('Library.name');
        $this->set(compact('books', 'heading'));
        $this->set('capacity', $this->Shelf->capacity());
    }

    public function view(int $id)
    {
        $book = $this->fetchTable('Books')->get($id);
        $reviews = $this->fetchTable('Reviews.Reviews')->find()->toArray();
        $this->set('book', $book);
        $this->set(['reviews' => $reviews]);
        $this->viewBuilder()->setLayout('default');

        return $this->render('view');
    }

    public function add()
    {
        $book = $this->Books->newEmptyEntity();
        if ($this->Books->save($book)) {
            $this->Flash->success(__('The book has been saved.'));

            return $this->redirect(['action' => 'index']);
        }
        $home = Router::url(['_name' => 'home']);
        $this->set(compact('book', 'home'));

        return $this->redirect(['controller' => 'Authors', 'action' => 'view', $book->author_id]);
    }

    public function reviews()
    {
        return $this->redirect(['plugin' => 'Reviews', 'controller' => 'Reviews', 'action' => 'index']);
    }
}
