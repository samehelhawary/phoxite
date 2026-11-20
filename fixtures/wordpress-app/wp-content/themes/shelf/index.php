<?php

get_header(is_singular(['book']) ? 'shop' : null);
get_header('shop');

echo esc_html($post->post_title);
foreach (get_the_terms($post, 'genre') as $genre) {
    echo esc_html($genre->name);
}
the_post_thumbnail('card');
get_template_part('template-parts/content', 'book');
get_template_part('template-parts/content');
wp_nav_menu(['theme_location' => 'primary']);

get_footer();
