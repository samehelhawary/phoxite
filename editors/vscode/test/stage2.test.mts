import assert from "node:assert/strict";
import { test } from "node:test";
import { OTHERS, conflictMessage, listed, seenKey } from "../src/conflicts.ts";
import { ownName, rewritten, withOwnCommand } from "../src/serverCommands.ts";
import { statusView } from "../src/status.ts";

const other = (id: string) => OTHERS.find((o) => o.id === id)!;

test("names are listed as a sentence would", () => {
  assert.equal(listed([]), "");
  assert.equal(listed(["A"]), "A");
  assert.equal(listed(["A", "B"]), "A and B");
  assert.equal(listed(["A", "B", "C"]), "A, B and C");
});

test("the warning says what each other extension doubles", () => {
  assert.equal(conflictMessage([]), undefined);
  assert.equal(
    conflictMessage([other("bmewburn.vscode-intelephense-client")]),
    "Phoxite: PHP Intelephense is enabled too, so completions and problems show twice. Disable what you don't need for this workspace.",
  );
  const several = conflictMessage([
    other("bmewburn.vscode-intelephense-client"),
    other("DEVSENSE.phptools-vscode"),
    other("laravel.vscode-laravel"),
    other("onecentlin.laravel-blade"),
  ]);
  assert.match(several ?? "", /PHP Intelephense and PHP \(DEVSENSE\) are enabled too/);
  assert.match(several ?? "", /Laravel also completes Laravel names/);
  assert.match(several ?? "", /Laravel Blade Snippets also defines the Blade language/);
  assert.match(
    conflictMessage([other("mblode.twig-language-2"), other("mblode.twig-language")]) ?? "",
    /Twig Language 2 and Twig Language also define the Twig language/,
  );
});

test("the warning is remembered for the extensions it named, in any order", () => {
  const a = other("laravel.vscode-laravel");
  const b = other("DEVSENSE.phptools-vscode");
  assert.equal(seenKey([a, b]), seenKey([b, a]));
  assert.equal(seenKey([b]), "devsense.phptools-vscode");
  assert.notEqual(seenKey([a]), seenKey([a, b]));
  // Every id is one the Marketplace lists once.
  assert.equal(new Set(OTHERS.map((o) => o.id.toLowerCase())).size, OTHERS.length);
});

test("the status item says what the server does", () => {
  const base = { indexing: false, laravel: false };
  assert.deepEqual(statusView({ ...base, server: "starting" }), {
    text: "Phoxite: starting",
    detail: "The language server is starting",
    busy: true,
    severity: "information",
  });
  const failed = statusView({ ...base, server: "failed", problem: "`phoxite.server.path` names x, which doesn't exist." });
  assert.equal(failed.severity, "error");
  assert.match(failed.detail, /doesn't exist/);
  assert.equal(statusView({ ...base, server: "stopped" }).severity, "error");
  assert.equal(statusView({ ...base, server: "running", indexing: true }).busy, true);
  assert.deepEqual(
    statusView({ server: "running", indexing: false, laravel: true, version: "0.3.0", indexed: "50 files, 312 declarations" }),
    { text: "Phoxite: Laravel", detail: "phoxite-lsp 0.3.0 · 50 files, 312 declarations", busy: false, severity: "information" },
  );
  assert.equal(statusView({ ...base, server: "running" }).text, "Phoxite");
});

test("the server's commands run under the extension's own names", () => {
  assert.equal(ownName("laravel.modelView"), "phoxite.server.laravel.modelView");
  assert.equal(rewritten("laravel.routeActions"), "phoxite.server.laravel.routeActions");
  // The extension's own commands and anyone else's are left alone.
  assert.equal(rewritten("phoxite.openLocation"), "phoxite.openLocation");
  assert.equal(rewritten("editor.action.rename"), "editor.action.rename");

  const lens = withOwnCommand({ command: { command: "laravel.runArtisan", title: "Run", arguments: ["inspire"] } });
  assert.deepEqual(lens.command, { command: "phoxite.server.laravel.runArtisan", title: "Run", arguments: ["inspire"] });
  assert.deepEqual(withOwnCommand({ command: undefined }), { command: undefined });
  assert.deepEqual(withOwnCommand({ command: "laravel.modelView" }), { command: "laravel.modelView" });
});
