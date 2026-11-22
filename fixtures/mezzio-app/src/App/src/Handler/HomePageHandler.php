<?php

namespace App\Handler;

use App\Service\Greeter;
use Mezzio\Helper\UrlHelper;
use Mezzio\Template\TemplateRendererInterface;

class HomePageHandler
{
    public function __construct(
        private TemplateRendererInterface $template,
        private UrlHelper $url,
        private Greeter $greeter,
    ) {
    }

    public function handle(): string
    {
        return $this->template->render('app::home-page', [
            'greeting' => $this->greeter->greet('reader'),
            'ping' => $this->url->generate('api.ping'),
            'visits' => 3,
        ]);
    }
}
