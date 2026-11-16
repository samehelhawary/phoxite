#!/usr/bin/env node
// Checks that everything that states the version states the same one: the Cargo
// workspace, the VS Code extension's manifest and its lock file, and a heading of the
// changelog. With a tag (`v0.3.0`) the tag has to state it too.
//
//   node scripts/check_versions.mjs [tag]
//
// An "Unreleased" section in the changelog is fine between releases; a release (a tag is
// given) needs the heading of its version.

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const read = (...parts) => fs.readFileSync(path.join(root, ...parts), "utf8");

/** The `version` of the `[workspace.package]` table of the root manifest. */
export function cargoVersion(manifest) {
  const table = manifest.split(/^\[/m).find((part) => part.startsWith("workspace.package]"));
  return table?.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
}

/** Whether the changelog has a heading for `version`: `## 0.3.0` or `## [0.3.0] - 2026-…`. */
export function changelogHas(changelog, version) {
  const escaped = version.replace(/[.+]/g, "\\$&");
  return new RegExp(`^##\\s+\\[?v?${escaped}\\]?(\\s|$)`, "m").test(changelog);
}

/** What disagrees, as sentences; empty when everything agrees. */
export function problems({ cargo, extension, lock, changelog, tag }) {
  const found = [];
  if (!cargo) return ["Cargo.toml has no [workspace.package] version"];
  if (extension !== cargo) found.push(`editors/vscode/package.json says ${extension}, Cargo.toml says ${cargo}`);
  if (lock !== undefined && lock !== extension) found.push(`editors/vscode/package-lock.json says ${lock}, package.json says ${extension}`);
  if (tag !== undefined) {
    if (tag !== `v${cargo}`) found.push(`the tag is ${tag}, the version is ${cargo} (expected v${cargo})`);
    if (!changelogHas(changelog, cargo)) found.push(`CHANGELOG.md has no heading for ${cargo}`);
  }
  return found;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const found = problems({
    cargo: cargoVersion(read("Cargo.toml")),
    extension: JSON.parse(read("editors", "vscode", "package.json")).version,
    lock: JSON.parse(read("editors", "vscode", "package-lock.json")).version,
    changelog: read("CHANGELOG.md"),
    tag: process.argv[2] || undefined,
  });
  for (const problem of found) console.error(problem);
  if (found.length) process.exit(1);
  console.log(`version ${cargoVersion(read("Cargo.toml"))} everywhere`);
}
