#!/usr/bin/env python3
"""Packages the VS Code extension with a server binary.

    python scripts/package_vscode.py --server target/release/phoxite-lsp [--target win32-x64]

Copies the binary into editors/vscode/server/ and runs `vsce package` (with
`--target` for a platform-specific VSIX). The VSIX lands in editors/vscode/.
"""

import argparse
import os
import shutil
import stat
import subprocess
import sys

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXTENSION = os.path.join(HERE, "editors", "vscode")

# VS Code's platform names for the Rust targets the release builds.
TARGETS = {
    "x86_64-pc-windows-msvc": "win32-x64",
    "aarch64-pc-windows-msvc": "win32-arm64",
    "x86_64-unknown-linux-gnu": "linux-x64",
    "aarch64-unknown-linux-gnu": "linux-arm64",
    "x86_64-unknown-linux-musl": "alpine-x64",
    "x86_64-apple-darwin": "darwin-x64",
    "aarch64-apple-darwin": "darwin-arm64",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--server", required=True, help="the phoxite-lsp binary")
    parser.add_argument("--target", help="a VS Code platform (win32-x64…) or a Rust target triple")
    args = parser.parse_args()
    if not os.path.isfile(args.server):
        sys.exit(f"{args.server} is not a file")
    target = TARGETS.get(args.target, args.target)
    windows = args.server.endswith(".exe") or (target or "").startswith("win32")
    out_dir = os.path.join(EXTENSION, "server")
    shutil.rmtree(out_dir, ignore_errors=True)
    os.makedirs(out_dir)
    binary = os.path.join(out_dir, "phoxite-lsp.exe" if windows else "phoxite-lsp")
    shutil.copy2(args.server, binary)
    os.chmod(binary, os.stat(binary).st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    npx = "npx.cmd" if os.name == "nt" else "npx"
    # esbuild bundles the client library into dist/extension.js.
    command = [npx, "vsce", "package", "--no-dependencies"]
    if target:
        command += ["--target", target]
    subprocess.run(command, cwd=EXTENSION, check=True)


if __name__ == "__main__":
    main()
