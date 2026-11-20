<?php

add_action('init', 'shelf_setup');

function shelf_setup(): void
{
    add_image_size('card', 300, 200);
    register_nav_menus(['primary' => 'Primary', 'footer' => 'Footer']);
    wp_register_script('shelf-app', get_theme_file_uri('/assets/app.js'));
    wp_enqueue_script('shelf-cart', get_theme_file_uri('/assets/app.js'), ['shelf-app']);
    wp_enqueue_script('shelf-app');
    locate_template(['inc/missing.php', 'template-parts/content.php'], true);
}
