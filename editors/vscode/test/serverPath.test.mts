import assert from "node:assert/strict";
import * as path from "node:path";
import test from "node:test";
import { bundledName, configuredServer, serverProblem } from "../src/serverPath.ts";

const context = { home: path.resolve("/home/me"), folder: path.resolve("/work/app") };

test("the setting names the server: a home folder, a path in the workspace, or a full path", () => {
  assert.equal(configuredServer("", context), undefined);
  assert.equal(configuredServer("   ", context), undefined);
  assert.equal(configuredServer(undefined, context), undefined);
  assert.equal(configuredServer("~/bin/phoxite-lsp", context), path.join(context.home, "bin", "phoxite-lsp"));
  assert.equal(configuredServer(" target/debug/phoxite-lsp ", context), path.resolve(context.folder, "target/debug/phoxite-lsp"));
  const absolute = path.resolve("/opt/phoxite-lsp");
  assert.equal(configuredServer(absolute, context), absolute);
  // No folder is open: a relative path is left for the system to find.
  assert.equal(configuredServer("phoxite-lsp", { home: context.home, folder: undefined }), "phoxite-lsp");
});

test("a server that isn't there is named before it is started", () => {
  const bundled = path.resolve("/ext/server", bundledName("linux"));
  const only = (file: string) => (candidate: string) => candidate === file;
  assert.equal(serverProblem(undefined, bundled, only(bundled)), null);
  assert.match(serverProblem(undefined, bundled, () => false) ?? "", /no language server for your platform/);
  const configured = path.resolve("/opt/phoxite-lsp");
  assert.equal(serverProblem(configured, bundled, only(configured)), null);
  // The setting wins: a bundled server doesn't hide a wrong path.
  assert.match(serverProblem(configured, bundled, only(bundled)) ?? "", /phoxite\.server\.path.*doesn't exist/);
  assert.equal(bundledName("win32"), "phoxite-lsp.exe");
  assert.equal(bundledName("darwin"), "phoxite-lsp");
});
