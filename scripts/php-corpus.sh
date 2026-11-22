#!/usr/bin/env bash
# Shallow-clone the PHP corpora used by the engine's ignored tests and benches into
# ${PHP_CORPUS_DIR:-../php-corpus}. Needs only git. A corpus already at its pinned tag is
# skipped; one at another tag is reported and left alone (delete it to re-clone).
#   scripts/php-corpus.sh                     clone what is missing
#   scripts/php-corpus.sh --check             only verify, exit 1 if anything is missing or off-tag
#   scripts/php-corpus.sh laravel-framework   only the corpora named (after --check too)
set -euo pipefail

cd "$(dirname "$0")/.."
DIR=${PHP_CORPUS_DIR:-../php-corpus}

# name  repository  pinned tag
CORPORA=(
    "laravel-framework https://github.com/laravel/framework.git v13.40.0"
    "symfony https://github.com/symfony/symfony.git v8.1.11"
    "wordpress https://github.com/WordPress/WordPress.git 7.1.4"
    "drupal https://github.com/drupal/drupal.git 11.2.0"
    "drupal7 https://github.com/drupal/drupal.git 7.103"
    "codeigniter https://github.com/codeigniter4/CodeIgniter4.git v4.6.5"
    "codeigniter3 https://github.com/bcit-ci/CodeIgniter.git 3.1.13"
    "yii https://github.com/yiisoft/yii2-app-basic.git 2.0.55"
    "yii-framework https://github.com/yiisoft/yii2-framework.git 2.0.55"
    "yii-advanced https://github.com/yiisoft/yii2-app-advanced.git 2.0.55"
    "cakephp https://github.com/cakephp/app.git 5.4.0"
    "cakephp-framework https://github.com/cakephp/cakephp.git 5.4.4"
    "cakephp4-framework https://github.com/cakephp/cakephp.git 4.6.6"
    "laminas https://github.com/laminas/laminas-mvc-skeleton.git 2.4.0"
    "mezzio https://github.com/mezzio/mezzio-skeleton.git 3.19.0"
    "laminas-mvc https://github.com/laminas/laminas-mvc.git 3.8.0"
    "laminas-view https://github.com/laminas/laminas-view.git 3.2.0"
    "laminas-view2 https://github.com/laminas/laminas-view.git 2.44.0"
    "laminas-servicemanager https://github.com/laminas/laminas-servicemanager.git 4.6.0"
    "laminas-router https://github.com/laminas/laminas-router.git 3.19.0"
    "mezzio-framework https://github.com/mezzio/mezzio.git 3.29.0"
    "mezzio-router https://github.com/mezzio/mezzio-router.git 4.3.0"
    "mezzio-template https://github.com/mezzio/mezzio-template.git 3.1.0"
    "mezzio-helpers https://github.com/mezzio/mezzio-helpers.git 5.21.0"
    "mezzio-laminasviewrenderer https://github.com/mezzio/mezzio-laminasviewrenderer.git 3.1.0"
    "psr-container https://github.com/php-fig/container.git 2.0.2"
    "laminas-form https://github.com/laminas/laminas-form.git 3.25.0"
    "laminas-validator https://github.com/laminas/laminas-validator.git 3.21.0"
    "laminas-filter https://github.com/laminas/laminas-filter.git 3.5.0"
    "laminas-hydrator https://github.com/laminas/laminas-hydrator.git 5.1.0"
    "laminas-inputfilter https://github.com/laminas/laminas-inputfilter.git 2.36.0"
    "laminas-navigation https://github.com/laminas/laminas-navigation.git 2.24.0"
    "magento https://github.com/magento/magento2.git 2.4.9"
    "joomla https://github.com/joomla/joomla-cms.git 6.2.0"
    "joomla-registry https://github.com/joomla-framework/registry.git 4.0.1"
    "joomla-di https://github.com/joomla-framework/di.git 4.0.0"
    "joomla-event https://github.com/joomla-framework/event.git 4.0.0"
    "joomla-database https://github.com/joomla-framework/database.git 4.0.0"
    "joomla-input https://github.com/joomla-framework/input.git 4.0.0"
)

check=0
if [[ "${1:-}" == --check ]]; then
    check=1
    shift
fi
wanted=("$@")
for name in "${wanted[@]}"; do
    known=0
    for entry in "${CORPORA[@]}"; do
        [[ "${entry%% *}" == "$name" ]] && known=1
    done
    if ((!known)); then
        echo "usage: $0 [--check] [laravel-framework|symfony|wordpress|drupal|drupal7|codeigniter|codeigniter3|yii|yii-framework|yii-advanced|cakephp|cakephp-framework|cakephp4-framework|laminas|mezzio|laminas-mvc|laminas-view|laminas-view2|laminas-servicemanager|laminas-router|mezzio-framework|mezzio-router|mezzio-template|mezzio-helpers|mezzio-laminasviewrenderer|psr-container|laminas-form|laminas-validator|laminas-filter|laminas-hydrator|laminas-inputfilter|laminas-navigation|magento|joomla|joomla-registry|joomla-di|joomla-event|joomla-database|joomla-input]..." >&2
        exit 2
    fi
done

command -v git >/dev/null || {
    echo "git is required" >&2
    exit 1
}
mkdir -p "$DIR"

status=0
for entry in "${CORPORA[@]}"; do
    read -r name url tag <<<"$entry"
    if ((${#wanted[@]})) && [[ " ${wanted[*]} " != *" $name "* ]]; then
        continue
    fi
    dest="$DIR/$name"
    if [[ -d "$dest/.git" ]]; then
        have=$(git -C "$dest" describe --tags --exact-match 2>/dev/null || echo "?")
        if [[ "$have" == "$tag" ]]; then
            echo "ok       $name $tag"
        else
            echo "off-tag  $name has $have, want $tag (delete $dest to re-clone)" >&2
            status=1
        fi
        continue
    fi
    if ((check)); then
        echo "missing  $name ($dest)" >&2
        status=1
        continue
    fi
    echo "clone    $name $tag"
    rm -rf "$dest.partial"
    git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$tag" "$url" "$dest.partial"
    mv "$dest.partial" "$dest"
    echo "ok       $name $tag"
done

if ((status == 0)); then
    count=$(find "$DIR" -name '*.php' -not -path '*/.git/*' | wc -l | tr -d ' ')
    echo "corpora ready in $DIR ($count .php files)"
fi
exit $status
