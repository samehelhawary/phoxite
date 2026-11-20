<?php

namespace App\Controller;

use Psr\Container\ContainerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Contracts\Translation\TranslatorInterface;

class BlogController extends AbstractController
{
    public function show(ContainerInterface $container, TranslatorInterface $translator): string
    {
        $mailer = $container->get('mailer.default');
        if (! $mailer->send('reader@example.com')) {
            return $this->redirectToRoute('home');
        }
        $title = $translator->trans('blog.title');

        return $this->render('blog/show.html.twig', ['title' => $title, 'link' => $this->generateUrl('blog_show')]);
    }
}
