<?php

namespace Reviews\Controller;

use Cake\Controller\Controller;

class ReviewsController extends Controller
{
    public function index(): void
    {
        $reviews = $this->fetchTable('Reviews.Reviews')->find()->toArray();
        $this->set('reviews', $reviews);
    }
}
