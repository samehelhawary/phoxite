<?php
/**
 * Plugin Name: Shop
 */

require_once __DIR__ . '/includes/class-shop-catalog.php';

add_action('init', 'shop_register');
add_action('save_post', 'shop_saved', 10, 2);
add_action('save_post_book', [Shop_Catalog::class, 'refresh']);
add_filter('the_title', function ($title, $id) {
    return $id > 0 ? strtoupper($title) : $title;
}, 10, 2);
add_action('shop_loaded', [new Shop_Catalog(), 'boot']);

function shop_register(): void
{
    register_post_type('book', ['public' => true, 'taxonomies' => ['genre']]);
    register_taxonomy('genre', ['book']);
    add_shortcode('price', 'shop_price');
    add_option('shop_currency', 'EUR');
    register_block_type('shop/price', ['render_callback' => 'shop_price']);
    register_rest_route('shop/v1', '/books', [
        'methods' => 'GET',
        'callback' => [Shop_Catalog::class, 'books'],
        'permission_callback' => 'shop_public',
    ]);

    /**
     * Fires when the shop is ready.
     *
     * @param Shop_Catalog $catalog The catalog.
     */
    do_action('shop_loaded', new Shop_Catalog());
}

function shop_saved($id, $post): void
{
    if ($id > 0 && has_block('shop/price')) {
        add_option('shop_last_saved', $post->post_title);
    }
}

function shop_price(): string
{
    return esc_html((string) get_option('shop_currency'));
}

function shop_public(): bool
{
    return true;
}

add_action('wp_ajax_shop_add', 'shop_add');

function shop_add(): string
{
    register_block_type(__DIR__ . '/blocks/stock');
    return has_block('shop/stock') ? rest_url('shop/v1/books') : '';
}
