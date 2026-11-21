#!/usr/bin/env bash
# Rebuild crates/phoxite/stubs/phpstorm-stubs.zip, the built-in PHP declarations the engine
# embeds, from the pinned JetBrains/phpstorm-stubs release. Needs git and cargo.
# The archive is checked in, so this only runs when moving to a new release: change TAG and
# COMMIT here and STUBS_TAG / STUBS_COMMIT in crates/phoxite/src/stubs/mod.rs together.
#   scripts/php-stubs.sh   clone into target/phpstorm-stubs-src (reused when on COMMIT) and pack
set -euo pipefail

cd "$(dirname "$0")/.."
TAG=v2026.3
COMMIT=912ae721089fc48534d3a5795997ee8a2802b8cd
SRC=target/phpstorm-stubs-src
OUT=crates/phoxite/stubs/phpstorm-stubs.zip

if [[ -d "$SRC/.git" ]] && [[ "$(git -C "$SRC" rev-parse HEAD)" == "$COMMIT" ]]; then
    echo "ok       phpstorm-stubs $TAG already in $SRC"
else
    if [[ -e "$SRC" ]]; then
        echo "$SRC exists but is not phpstorm-stubs $TAG; delete it and run again" >&2
        exit 1
    fi
    git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$TAG" \
        https://github.com/JetBrains/phpstorm-stubs.git "$SRC"
    have=$(git -C "$SRC" rev-parse HEAD)
    if [[ "$have" != "$COMMIT" ]]; then
        echo "tag $TAG is at $have, expected $COMMIT" >&2
        exit 1
    fi
fi

cargo run --quiet -p phoxite --example pack_stubs -- "$SRC" "$OUT"
