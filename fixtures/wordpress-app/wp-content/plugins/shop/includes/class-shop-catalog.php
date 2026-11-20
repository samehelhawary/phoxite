<?php

class Shop_Catalog
{
    public function boot(): void
    {
        if (has_action('shop_loaded') && shortcode_exists('price')) {
            add_action('admin_init', [$this, 'boot']);
        }
    }

    public static function refresh($id, $post): string
    {
        return $id . $post->post_title;
    }

    /** @return list<object> */
    public static function books(WP_REST_Request $request): array
    {
        global $wpdb;
        $query = $wpdb->prepare("SELECT * FROM {$wpdb->prefix}posts WHERE post_type = %s", $request->get_param('type'));
        return $wpdb->get_results($query);
    }

    /** @return list<WP_Post> */
    public function recent(): array
    {
        return get_posts(['post_type' => ['book'], 'numberposts' => 5]);
    }
}
