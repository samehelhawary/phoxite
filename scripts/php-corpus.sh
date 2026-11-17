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
        echo "usage: $0 [--check] [laravel-framework|symfony|wordpress]..." >&2
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
