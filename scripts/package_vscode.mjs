#!/usr/bin/env node
// Packages the VS Code extension with a server binary.
//
//   node scripts/package_vscode.mjs --server target/release/phoxite-lsp [--target win32-x64]
//
// Copies the binary into editors/vscode/server/, and the changelog, the notice and the
// third-party licences next to the extension's manifest, then runs `vsce package` (with
// `--target` for a platform-specific VSIX). The VSIX lands in editors/vscode/.
//
// `--list` prints the files the package would hold instead of writing it.
//
// A version with an odd minor (0.5.x) is packaged as a pre-release: the stores offer it
// only to who asked for pre-releases. Stable versions have even minors.

import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const extension = path.join(root, "editors", "vscode");

// VS Code's platform names for the Rust targets the release builds.
const TARGETS = {
  "x86_64-pc-windows-msvc": "win32-x64",
  "aarch64-pc-windows-msvc": "win32-arm64",
  "x86_64-unknown-linux-gnu": "linux-x64",
  "aarch64-unknown-linux-gnu": "linux-arm64",
  "x86_64-unknown-linux-musl": "alpine-x64",
  "x86_64-apple-darwin": "darwin-x64",
  "aarch64-apple-darwin": "darwin-arm64",
};

/** What travels with the extension: the Marketplace shows the changelog, and Apache-2.0 has the notice go with the work. */
const COPIED = ["CHANGELOG.md", "NOTICE", "THIRD_PARTY_LICENSES.md"];

/** Whether `version` is a pre-release: its minor is odd (the convention of the VS Code Marketplace). */
export function isPreRelease(version) {
  return Number(version.split(".")[1]) % 2 === 1;
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function options(argv) {
  const found = { list: false };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--list") found.list = true;
    else if (arg === "--server" || arg === "--target") {
      const value = argv[++i];
      if (!value) fail(`${arg} needs a value`);
      found[arg.slice(2)] = value;
    } else fail(`unknown argument ${arg}\nusage: node scripts/package_vscode.mjs --server <phoxite-lsp> [--target <platform>] [--list]`);
  }
  if (!found.server) fail("--server <the phoxite-lsp binary> is required");
  return found;
}

const { server, target: given, list } = options(process.argv.slice(2));
if (!fs.existsSync(server) || !fs.statSync(server).isFile()) fail(`${server} is not a file`);
const target = TARGETS[given] ?? given;
const windows = server.endsWith(".exe") || (target ?? "").startsWith("win32");

const serverDir = path.join(extension, "server");
fs.rmSync(serverDir, { recursive: true, force: true });
fs.mkdirSync(serverDir, { recursive: true });
const binary = path.join(serverDir, windows ? "phoxite-lsp.exe" : "phoxite-lsp");
fs.copyFileSync(server, binary);
fs.chmodSync(binary, 0o755);
for (const name of COPIED) fs.copyFileSync(path.join(root, name), path.join(extension, name));

// esbuild bundles the client library into dist/extension.js, so no dependencies are packed.
const { version } = JSON.parse(fs.readFileSync(path.join(extension, "package.json"), "utf8"));
const args = ["vsce", list ? "ls" : "package", "--no-dependencies"];
if (!list) {
  if (target) args.push("--target", target);
  if (isPreRelease(version)) args.push("--pre-release");
  // The README's pictures and links are served from the repository, at this version's tag:
  // the extension is not at the repository's root, where `vsce` would look.
  const repository = "https://github.com/samehelhawary/phoxite";
  args.push("--baseImagesUrl", `${repository}/raw/v${version}/editors/vscode`);
  args.push("--baseContentUrl", `${repository}/blob/v${version}/editors/vscode`);
}
const run = spawnSync(process.platform === "win32" ? "npx.cmd" : "npx", args, {
  cwd: extension,
  stdio: "inherit",
  // `npx.cmd` is a batch file: Node runs those through a shell only.
  shell: process.platform === "win32",
});
if (run.error) fail(String(run.error));
process.exit(run.status ?? 1);
