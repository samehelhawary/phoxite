<?php

namespace Application\Controller;

use Application\Service\Catalog;
use Laminas\Mvc\Controller\AbstractActionController;
use Laminas\View\Model\ViewModel;

class BookController extends AbstractActionController
{
    public function __construct(private Catalog $catalog)
    {
    }

    public function indexAction()
    {
        return new ViewModel([
            'books' => $this->catalog->newest(),
            'heading' => $this->catalog->title(),
            'capacity' => $this->shelf(),
        ]);
    }

    public function viewAction()
    {
        $book = $this->catalog->find(1);
        if ($book === null) {
            return $this->redirect()->toRoute('home');
        }
        $view = new ViewModel();
        $view->setVariable('book', $book);
        $view->setTemplate('application/book/view');
        $related = new ViewModel(['count' => 2]);
        $related->setTemplate('partial/related');
        $view->addChild($related, 'related');

        return $view;
    }

    public function firstAction()
    {
        $this->layout()->setVariable('section', 'Books');

        return $this->redirect()->toRoute('book/view', ['action' => 'view', 'id' => 1]);
    }
}
