<?php

/*
 * A stand-in for WordPress's API, written for this fixture: function names and
 * parameters only.
 */

class WP_Post
{
    /** @var int */
    public $ID = 0;
    /** @var string */
    public $post_title = '';
    /** @var string */
    public $post_type = 'post';
}

class WP_Term
{
    /** @var string */
    public $name = '';
    /** @var string */
    public $slug = '';
}

class WP_Query
{
    /** @param array<string, mixed>|string $query */
    public function __construct($query = '')
    {
    }

    public function have_posts(): bool
    {
        return false;
    }
}

class wpdb
{
    /** @var string */
    public $prefix = 'wp_';

    public function prepare(string $query, mixed ...$args): string
    {
        return $query . count($args);
    }

    /** @return list<object> */
    public function get_results(string $query): array
    {
        return [(object) ['query' => $query]];
    }
}

class WP_REST_Request
{
    public function get_param(string $key): mixed
    {
        return $key;
    }
}

/** @param callable $callback */
function add_action(string $hook_name, $callback, int $priority = 10, int $accepted_args = 1): bool
{
    return $hook_name !== '' && is_callable($callback) && $priority >= 0 && $accepted_args >= 0;
}

/** @param callable $callback */
function add_filter(string $hook_name, $callback, int $priority = 10, int $accepted_args = 1): bool
{
    return add_action($hook_name, $callback, $priority, $accepted_args);
}

function has_action(string $hook_name): bool
{
    return $hook_name !== '';
}

function do_action(string $hook_name, mixed ...$arg): void
{
}

function apply_filters(string $hook_name, mixed $value, mixed ...$args): mixed
{
    return $value;
}

/** @param callable $callback */
function add_shortcode(string $tag, $callback): void
{
}

function shortcode_exists(string $tag): bool
{
    return $tag !== '';
}

/** @param array<string, mixed> $args */
function register_post_type(string $post_type, array $args = []): void
{
}

/**
 * @param string|list<string>  $object_type
 * @param array<string, mixed> $args
 */
function register_taxonomy(string $taxonomy, $object_type, array $args = []): void
{
}

/** @param string|list<string> $post_types */
function is_singular($post_types = ''): bool
{
    return $post_types !== '';
}

/**
 * @param array<string, mixed> $args
 * @return list<WP_Post>
 */
function get_posts(array $args = []): array
{
    return [];
}

/** @return list<WP_Term> */
function get_the_terms(WP_Post $post, string $taxonomy): array
{
    return [];
}

/** @param list<string> $deps */
function wp_register_script(string $handle, string $src, array $deps = []): void
{
}

/** @param list<string> $deps */
function wp_enqueue_script(string $handle, string $src = '', array $deps = []): void
{
}

/** @param list<string> $deps */
function wp_enqueue_style(string $handle, string $src = '', array $deps = []): void
{
}

function add_option(string $option, mixed $value = ''): void
{
}

function get_option(string $option, mixed $default_value = false): mixed
{
    return $default_value;
}

function add_image_size(string $name, int $width = 0, int $height = 0): void
{
}

function the_post_thumbnail(string $size = 'post-thumbnail'): void
{
}

/** @param array<string, string> $locations */
function register_nav_menus(array $locations = []): void
{
}

/** @param array<string, mixed> $args */
function wp_nav_menu(array $args = []): void
{
}

/** @param array<string, mixed> $args */
function register_block_type(string $block_type, array $args = []): void
{
}

function has_block(string $block_name): bool
{
    return $block_name !== '';
}

/** @param array<string, mixed> $args */
function register_rest_route(string $route_namespace, string $route, array $args = []): void
{
}

function get_template_part(string $slug, ?string $name = null): void
{
}

function get_header(?string $name = null): void
{
}

function get_footer(?string $name = null): void
{
}

/** @param string|list<string> $template_names */
function locate_template($template_names, bool $load = false): string
{
    return '';
}

function get_theme_file_uri(string $file = ''): string
{
    return $file;
}

function esc_html(string $text): string
{
    return $text;
}

/**
 * @global wpdb         $wpdb The database.
 * @global WP_Post|null $post The current post.
 */
function wp_globals(): void
{
    global $wpdb, $post;
}

/**
 * @global WP_Query $wp_query The main query.
 */
function wp_save(WP_Post $saved, bool $update): string
{
    global $wp_query;

    /**
     * Fires once a post has been saved.
     *
     * @param int     $post_id The post's ID.
     * @param WP_Post $post    The post.
     * @param bool    $update  Whether it existed.
     */
    do_action('save_post', $saved->ID, $saved, $update);

    /**
     * Fires once a post of one type has been saved.
     *
     * @param int     $post_id The post's ID.
     * @param WP_Post $post    The post.
     */
    do_action("save_post_{$saved->post_type}", $saved->ID, $saved);

    /**
     * Filters a post's title.
     *
     * @param string $title The title.
     * @param int    $id    The post's ID.
     */
    return apply_filters('the_title', $saved->post_title, $saved->ID);
}

function rest_url(string $path = ''): string
{
    return $path;
}

function wp_ajax(string $action): void
{
    /**
     * Fires for the AJAX request of an action.
     */
    do_action('wp_ajax_' . $action);
}
