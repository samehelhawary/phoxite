// The end-to-end checks, run inside VS Code by run.js. Each check uses VS Code's
// own provider commands, so it goes through the extension and the server as a
// user's action would.

const assert = require("node:assert/strict");
const path = require("node:path");
const vscode = require("vscode");

const project = process.env.PHOXITE_E2E_PROJECT;
const file = (rel) => vscode.Uri.file(path.join(project, rel));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Retries `f` until `done` accepts its result (the index may still be building). */
async function until(label, f, done, timeout = 90_000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await f();
    if (done(last)) return last;
    await sleep(300);
  }
  throw new Error(`${label}: gave up; last answer ${JSON.stringify(last)?.slice(0, 300)}`);
}

async function open(rel) {
  const doc = await vscode.workspace.openTextDocument(file(rel));
  await vscode.window.showTextDocument(doc);
  return doc;
}

function at(doc, needle, offset = 0) {
  const index = doc.getText().indexOf(needle);
  assert.ok(index >= 0, `${needle} not in ${doc.uri.fsPath}`);
  return doc.positionAt(index + offset);
}

const hoverText = (hovers) =>
  (hovers ?? []).flatMap((h) => h.contents.map((c) => (typeof c === "string" ? c : c.value))).join("\n");

const checks = [
  ["the extension activates", async () => {
    const ext = vscode.extensions.getExtension("phoxite.phoxite");
    assert.ok(ext, "phoxite.phoxite is not installed");
    await ext.activate();
    const commands = await vscode.commands.getCommands(true);
    for (const id of ["phoxite.restart", "phoxite.rescan", "laravel.routeActions", "phoxite.openLocation"]) {
      assert.ok(commands.includes(id), `${id} is not registered`);
    }
  }],
  ["hover on a class", async () => {
    const doc = await open("routes/web.php");
    const pos = at(doc, "HomeController::class", 2);
    const hovers = await until("hover", () => vscode.commands.executeCommand("vscode.executeHoverProvider", doc.uri, pos),
      (h) => hoverText(h).includes("HomeController"));
    return hoverText(hovers).split("\n").find((l) => l.includes("class")) ?? "";
  }],
  ["go to a view from Route::view", async () => {
    const doc = await open("routes/web.php");
    const pos = at(doc, "'about')", 2);
    const found = await until("definition", () => vscode.commands.executeCommand("vscode.executeDefinitionProvider", doc.uri, pos),
      (l) => (l ?? []).length > 0);
    const target = (found[0].uri ?? found[0].targetUri).fsPath;
    assert.ok(target.endsWith(path.join("views", "about.blade.php")), target);
    return path.relative(project, target);
  }],
  ["route URI hints", async () => {
    const doc = await open("routes/web.php");
    const range = new vscode.Range(0, 0, doc.lineCount, 0);
    const hints = await until("inlay hints", () => vscode.commands.executeCommand("vscode.executeInlayHintProvider", doc.uri, range),
      (h) => (h ?? []).length > 0);
    const labels = hints.map((h) => (typeof h.label === "string" ? h.label : h.label.map((p) => p.value).join("")));
    return `${hints.length} hints: ${labels.slice(0, 4).join(", ")}`;
  }],
  ["route code lenses on a controller", async () => {
    const doc = await open("app/Http/Controllers/PostController.php");
    const lenses = await until("code lens", () => vscode.commands.executeCommand("vscode.executeCodeLensProvider", doc.uri, 20),
      (l) => (l ?? []).some((x) => x.command?.command === "laravel.routeActions"));
    return lenses.map((l) => l.command?.title).filter(Boolean).slice(0, 3).join(" | ");
  }],
  ["route name completion", async () => {
    const doc = await open("app/Http/Controllers/PostController.php");
    const edit = new vscode.WorkspaceEdit();
    const line = doc.lineCount - 1;
    const end = doc.lineAt(line).range.end;
    edit.insert(doc.uri, end, "\nfunction probe() { return route(''); }\n");
    await vscode.workspace.applyEdit(edit);
    const pos = at(doc, "route('')", 7);
    const list = await until("completion", () => vscode.commands.executeCommand("vscode.executeCompletionItemProvider", doc.uri, pos),
      (l) => (l?.items ?? []).some((i) => (typeof i.label === "string" ? i.label : i.label.label) === "posts.show"));
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    return `${list.items.length} items, includes posts.show`;
  }],
  ["member completion", async () => {
    const doc = await open("app/Http/Controllers/PostController.php");
    const text = doc.getText();
    const edit = new vscode.WorkspaceEdit();
    const brace = text.lastIndexOf("}");
    edit.insert(doc.uri, doc.positionAt(brace), "    public function probe(\\App\\Models\\Post $post) { $post->; }\n");
    await vscode.workspace.applyEdit(edit);
    const pos = at(doc, "$post->;", 7);
    const label = (i) => (typeof i.label === "string" ? i.label : i.label.label);
    const list = await until("completion", () => vscode.commands.executeCommand("vscode.executeCompletionItemProvider", doc.uri, pos),
      (l) => (l?.items ?? []).some((i) => label(i) === "author"));
    const labels = list.items.map(label);
    await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    // VS Code's own basic PHP suggestions are off (configurationDefaults).
    assert.ok(!labels.includes("$_GET"), `built-in suggestions mixed in: ${labels.slice(0, 10).join(", ")}`);
    return labels.filter((l) => ["author", "comments", "tags", "title", "save"].includes(l)).join(", ");
  }],
  ["problems and a quick-fix", async () => {
    const uri = file("app/Broken.php");
    await vscode.workspace.fs.writeFile(uri, Buffer.from("<?php\nnamespace App;\n\nfunction broken(): void {\n    new UnknownThing();\n    view('no.such.view');\n}\n"));
    const doc = await open("app/Broken.php");
    const problems = await until("diagnostics", async () => vscode.languages.getDiagnostics(uri), (d) => d.length >= 2);
    const range = problems.find((d) => d.message.includes("no.such.view"))?.range ?? problems[0].range;
    const actions = await vscode.commands.executeCommand("vscode.executeCodeActionProvider", doc.uri, range);
    return `${problems.map((d) => `[${d.code}] ${d.message}`).join("; ")} — fixes: ${(actions ?? []).map((a) => a.title).join(", ")}`;
  }],
  ["find usages of a route name", async () => {
    const doc = await open("routes/web.php");
    const pos = at(doc, "'posts.create'", 3);
    const refs = await until("references", () => vscode.commands.executeCommand("vscode.executeReferenceProvider", doc.uri, pos),
      (r) => (r ?? []).length > 0);
    return refs.map((r) => `${path.relative(project, r.uri.fsPath)}:${r.range.start.line + 1}`).join(", ");
  }],
  ["rename a class", async () => {
    const doc = await open("app/Http/Controllers/HomeController.php");
    const pos = at(doc, "class HomeController", 7);
    const edit = await vscode.commands.executeCommand("vscode.executeDocumentRenameProvider", doc.uri, pos, "StartController");
    assert.ok(edit, "no edit");
    const files = edit.entries().map(([uri, edits]) => `${path.relative(project, uri.fsPath)} (${edits.length})`);
    assert.ok(files.some((f) => f.startsWith(path.join("routes", "web.php"))), files.join(", "));
    return files.join(", ");
  }],
  ["Blade: language, outline, config hover", async () => {
    const doc = await open("resources/views/layouts/app.blade.php");
    assert.equal(doc.languageId, "blade");
    const symbols = await vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", doc.uri);
    const configAt = doc.getText().indexOf("config('");
    let hover = "";
    if (configAt >= 0) {
      const pos = doc.positionAt(configAt + 9);
      const hovers = await until("blade hover", () => vscode.commands.executeCommand("vscode.executeHoverProvider", doc.uri, pos),
        (h) => hoverText(h).length > 0, 30_000).catch(() => []);
      hover = hoverText(hovers).split("\n")[0];
    }
    return `languageId blade, ${(symbols ?? []).length} top-level symbols${hover ? `, hover: ${hover}` : ""}`;
  }],
  ["format a PHP document", async () => {
    const uri = file("app/Messy.php");
    await vscode.workspace.fs.writeFile(uri, Buffer.from("<?php\nnamespace App;\nclass Messy{public function a( $x ){return $x+1;}}\n"));
    const doc = await open("app/Messy.php");
    const edits = await vscode.commands.executeCommand("vscode.executeFormatDocumentProvider", doc.uri, { tabSize: 4, insertSpaces: true });
    assert.ok(Array.isArray(edits) && edits.length > 0, "no edits");
    const result = new vscode.WorkspaceEdit();
    result.set(doc.uri, edits);
    await vscode.workspace.applyEdit(result);
    return doc.getText().split("\n").slice(2, 6).join(" ⏎ ");
  }],
  ["workspace symbols", async () => {
    const found = await vscode.commands.executeCommand("vscode.executeWorkspaceSymbolProvider", "PostCont");
    assert.ok((found ?? []).some((s) => s.name === "PostController"), JSON.stringify(found?.slice(0, 3)));
    return `${found.length} symbols`;
  }],
  ["semantic tokens", async () => {
    const doc = await open("app/Http/Controllers/PostController.php");
    const tokens = await vscode.commands.executeCommand("vscode.provideDocumentSemanticTokens", doc.uri);
    assert.ok(tokens && tokens.data.length > 0, "no tokens");
    return `${tokens.data.length / 5} tokens`;
  }],
  ["call hierarchy", async () => {
    const doc = await open("app/Http/Controllers/PostController.php");
    const index = doc.getText().indexOf("function ");
    const pos = doc.positionAt(index + 10);
    const items = await vscode.commands.executeCommand("vscode.prepareCallHierarchy", doc.uri, pos);
    assert.ok((items ?? []).length > 0, "no item");
    const outgoing = await vscode.commands.executeCommand("vscode.provideOutgoingCalls", items[0]);
    return `${items[0].name}: ${(outgoing ?? []).length} outgoing calls`;
  }],
];

exports.run = async function run() {
  const results = [];
  for (const [name, check] of checks) {
    const started = Date.now();
    try {
      const detail = await check();
      results.push({ name, ok: true, ms: Date.now() - started, detail });
    } catch (e) {
      results.push({ name, ok: false, ms: Date.now() - started, detail: e.message });
    }
  }
  for (const r of results) {
    console.log(`${r.ok ? "PASS" : "FAIL"}  ${r.name} (${r.ms} ms)${r.detail ? `\n      ${r.detail}` : ""}`);
  }
  const failed = results.filter((r) => !r.ok).length;
  console.log(`\n${results.length - failed} passed, ${failed} failed`);
  if (failed) throw new Error(`${failed} end-to-end checks failed`);
};
