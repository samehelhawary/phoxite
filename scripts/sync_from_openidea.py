#!/usr/bin/env python3
"""Copies the engine from an OpenIDEA checkout into this repository.

Until OpenIDEA depends on this repository (docs/openidea.md), the engine is
developed in OpenIDEA's `crates/ide-php` and copied here:

    python scripts/sync_from_openidea.py ../openidea

It replaces `crates/php-engine/{src,tests,examples,benches,stubs}`, the
fixtures and `docs/engine`, renames the crate (`ide_php` to `php_engine`,
snapshot files included), leaves out OpenIDEA's own tests, and records the
source commit in `crates/php-engine/SYNCED_FROM`. `crates/php-engine/Cargo.toml`
is this repository's own and is not touched.
"""

import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# OpenIDEA-only, relative to the crate: drives Intelephense through OpenIDEA's LSP client.
SKIP = {"tests/differential.rs"}
TEXT = (".rs", ".md", ".snap", ".toml", ".json", ".php", ".txt")


def copy_tree(src, dst, rename=False, prefix=""):
    if os.path.exists(dst):
        shutil.rmtree(dst)
    for root, dirs, files in os.walk(src):
        dirs[:] = [d for d in dirs if d not in ("target", ".git")]
        rel_dir = os.path.relpath(root, src)
        for name in files:
            rel = os.path.normpath(os.path.join(rel_dir, name)).replace(os.sep, "/")
            if f"{prefix}{rel}" in SKIP or name.endswith(".snap.new"):
                continue
            out_name = name.replace("ide_php__", "php_engine__") if rename else name
            out = os.path.join(dst, rel_dir, out_name)
            os.makedirs(os.path.dirname(out), exist_ok=True)
            if rename and name.endswith(TEXT):
                with open(os.path.join(root, name), "rb") as f:
                    text = f.read().decode("utf-8")
                text = re.sub(r"\bide_php\b", "php_engine", text)
                text = text.replace("crates/ide-php/", "crates/php-engine/")
                with open(out, "wb") as f:
                    f.write(text.encode("utf-8"))
            else:
                shutil.copy2(os.path.join(root, name), out)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    openidea = os.path.abspath(sys.argv[1])
    crate = os.path.join(openidea, "crates", "ide-php")
    if not os.path.isdir(crate):
        sys.exit(f"{crate} is not a directory")
    target = os.path.join(HERE, "crates", "php-engine")
    for part in ("src", "tests", "examples", "benches", "stubs"):
        copy_tree(os.path.join(crate, part), os.path.join(target, part), rename=True, prefix=f"{part}/")
    # The Laravel generators' default templates live in OpenIDEA's frontend
    # (its file templates show them); here they are part of the crate.
    templates = os.path.join(target, "templates", "laravel")
    copy_tree(os.path.join(openidea, "src", "features", "templates", "files", "laravel"), templates)
    template_rs = os.path.join(target, "src", "frameworks", "laravel", "generate", "template.rs")
    with open(template_rs, "rb") as f:
        text = f.read().decode("utf-8")
    old = '"../../../../../../src/features/templates/files/laravel/"'
    if old not in text:
        sys.exit(f"{template_rs} no longer includes the templates from {old}; update this script")
    with open(template_rs, "wb") as f:
        f.write(text.replace(old, '"../../../../templates/laravel/"').encode("utf-8"))
    for fixture in ("php-library", "laravel-app", "laravel-stubs"):
        copy_tree(os.path.join(openidea, "fixtures", fixture), os.path.join(HERE, "fixtures", fixture))
    copy_tree(os.path.join(openidea, "docs", "php-engine"), os.path.join(HERE, "docs", "engine"), rename=True)
    commit = subprocess.run(
        ["git", "-C", openidea, "rev-parse", "HEAD"], capture_output=True, text=True, check=False
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "-C", openidea, "status", "--porcelain", "--", "crates/ide-php", "fixtures", "docs/php-engine"],
        capture_output=True, text=True, check=False,
    ).stdout.strip()
    with open(os.path.join(target, "SYNCED_FROM"), "w", newline="\n") as f:
        f.write(f"openidea {commit}{' (with uncommitted changes)' if dirty else ''}\n")
    print(f"synced from {openidea} at {commit or 'an unknown commit'}")


if __name__ == "__main__":
    main()
