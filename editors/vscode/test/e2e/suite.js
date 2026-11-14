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

/** The extension's Laravel requests (what `activate` answers). */
const laravel = () => vscode.extensions.getExtension("phoxite.phoxite").exports.laravel;

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
  ["Laravel: the project is recognised and its commands are there", async () => {
    await until("the Laravel status", () => laravel().refreshStatus(), (active) => active === true);
    const commands = await vscode.commands.getCommands(true);
    for (const id of [
      "phoxite.laravel.generate", "phoxite.laravel.newModel", "phoxite.laravel.modelView", "phoxite.laravel.artisan",
      "phoxite.laravel.routes.goto", "phoxite.laravel.editTemplate", "laravel.modelView", "laravel.runArtisan",
      "laravel.createTranslation",
    ]) {
      assert.ok(commands.includes(id), `${id} is not registered`);
    }
  }],
  ["Laravel: the routes list, and going to a route", async () => {
    const routes = await until("routes", () => laravel().routes(), (r) => r.length > 0);
    const route = routes.find((r) => r.name === "posts.show");
    assert.ok(route, "no posts.show");
    await vscode.commands.executeCommand("phoxite.laravel.route.goto", { route });
    const editor = vscode.window.activeTextEditor;
    assert.ok(editor.document.uri.fsPath.endsWith(path.join("routes", "web.php")), editor.document.uri.fsPath);
    assert.equal(editor.selection.start.line, route.range.start.line);
    return `${routes.length} routes; posts.show at web.php:${route.range.start.line + 1}`;
  }],
  ["Laravel: a class generator writes its file", async () => {
    const ctx = await until("the generators", () => laravel().generationContext(),
      (c) => (c?.models ?? []).includes("App\\Models\\Post"));
    assert.ok(ctx.generators.some((g) => g.id === "controller"), "no controller generator");
    const created = await laravel().apply(await laravel().generateClass(
      { generator: "enum", name: "Visibility", module: "", backing: "string", inputs: {} }));
    assert.equal(created.length, 1);
    const text = Buffer.from(await vscode.workspace.fs.readFile(created[0])).toString();
    assert.ok(text.includes("enum Visibility: string"), text);
    const doc = vscode.workspace.textDocuments.find((d) => d.uri.toString() === created[0].toString());
    assert.ok(!doc?.isDirty, "the new file was left unsaved");
    return `${path.relative(project, created[0].fsPath)}; ${ctx.generators.length} generators, ${ctx.models.length} models`;
  }],
  ["Laravel: New Model writes the model and what it ticks", async () => {
    const created = await laravel().apply(await laravel().newModel({
      name: "Widget", module: "", fields: [{ name: "label", ty: "string" }],
      relations: [{ kind: "belongsTo", model: "App\\Models\\User", name: "" }], indexes: [],
      timestamps: true, softDeletes: false, migration: true, factory: true, seeder: false,
      controller: "none", formRequests: false, resource: false, policy: false,
    }));
    const names = created.map((uri) => path.basename(uri.fsPath));
    assert.ok(names.includes("Widget.php") && names.includes("WidgetFactory.php"), names.join(", "));
    assert.ok(names.some((n) => /^\d{4}_\d\d_\d\d_\d{6}_create_widgets_table\.php$/.test(n)), names.join(", "));
    for (const uri of created) await vscode.workspace.fs.stat(uri);
    return names.join(", ");
  }],
  ["Laravel: the New Model form and the model view open", async () => {
    const tabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs)
      .filter((t) => t.input instanceof vscode.TabInputWebview);
    await vscode.commands.executeCommand("phoxite.laravel.newModel");
    const form = await until("the form", async () => tabs().find((t) => t.input.viewType.includes("phoxite.newModel")), Boolean, 20_000);
    await vscode.window.tabGroups.close(form);
    await vscode.commands.executeCommand("laravel.modelView", "App\\Models\\Post");
    const view = await until("the view", async () => tabs().find((t) => t.input.viewType.includes("phoxite.modelView")), Boolean, 20_000);
    // The lens on the same model shows the open view again.
    await vscode.commands.executeCommand("laravel.modelView", "App\\Models\\Post");
    assert.equal(tabs().filter((t) => t.input.viewType.includes("phoxite.modelView")).length, 1);
    const label = view.label;
    await vscode.window.tabGroups.close(view);
    return `tabs: ${form.label}, ${label}`;
  }],
  ["Laravel: the model view's data, and adding columns", async () => {
    const before = await laravel().modelView("App\\Models\\Post");
    assert.equal(before.table, "posts");
    const created = await laravel().apply(await laravel().addColumns(before.class,
      [{ name: "subtitle", ty: "string", nullable: true }], true));
    assert.ok(/_add_subtitle_to_posts_table\.php$/.test(created[0].fsPath), created[0].fsPath);
    // The server sees the migration once the file watcher reports it.
    const after = await until("the new column", () => laravel().modelView(before.class),
      (v) => v.columns.some((c) => c.name === "subtitle" && c.fillable));
    return `${before.columns.length} → ${after.columns.length} columns, ${after.relations.length} relations`;
  }],
  ["Laravel: Artisan commands, their lens and their tasks", async () => {
    const commands = await laravel().artisanCommands();
    assert.ok(commands.some((c) => c.name === "posts:publish" && c.source === "class"), "no posts:publish");
    const doc = await open("app/Console/Commands/PublishScheduledPosts.php");
    const lenses = await until("the lens", () => vscode.commands.executeCommand("vscode.executeCodeLensProvider", doc.uri, 20),
      (l) => (l ?? []).some((x) => x.command?.command === "laravel.runArtisan"));
    const lens = lenses.find((l) => l.command?.command === "laravel.runArtisan");
    assert.deepEqual(lens.command.arguments, ["posts:publish"]);
    const tasks = await vscode.tasks.fetchTasks({ type: "artisan" });
    const task = tasks.find((t) => t.definition.command === "posts:publish");
    assert.ok(task, `no task in ${tasks.map((t) => t.name).join(", ")}`);
    assert.equal(task.execution.commandLine, "php artisan posts:publish");
    return `${commands.length} commands; lens "${lens.command.title}"; ${tasks.length} tasks`;
  }],
  ["Laravel: Create translation key, the action and the edit", async () => {
    const uri = file("app/Translated.php");
    await vscode.workspace.fs.writeFile(uri, Buffer.from("<?php\nnamespace App;\n\nfunction translated(): string {\n    return __('messages.from_the_test');\n}\n"));
    const doc = await open("app/Translated.php");
    const pos = at(doc, "messages.from_the_test", 3);
    const actions = await until("the action", () => vscode.commands.executeCommand("vscode.executeCodeActionProvider", doc.uri, new vscode.Range(pos, pos)),
      (a) => (a ?? []).some((x) => x.command?.command === "laravel.createTranslation"));
    const action = actions.find((a) => a.command?.command === "laravel.createTranslation");
    const request = JSON.parse(action.command.arguments[0]);
    assert.deepEqual(request.locales.map((l) => `${l.locale}:${l.exists}`), ["en:false", "fr:false"]);
    await laravel().apply(await laravel().createTranslation(request.key, [{ locale: "fr", text: "Depuis le test" }]));
    const fr = Buffer.from(await vscode.workspace.fs.readFile(file("lang/fr/messages.php"))).toString();
    const en = Buffer.from(await vscode.workspace.fs.readFile(file("lang/en/messages.php"))).toString();
    assert.ok(fr.includes("Depuis le test"), fr);
    assert.ok(!en.includes("from_the_test"), en);
    return `"${action.title}"; written to lang/fr/messages.php only`;
  }],
  ["Laravel: an edited template replaces the default", async () => {
    const templates = await laravel().templates();
    const template = templates.find((t) => t.name === "Laravel Enum");
    assert.ok(template && !template.edited, JSON.stringify(template));
    await vscode.workspace.fs.writeFile(vscode.Uri.file(template.path), Buffer.from(`${template.body}// from my template\n`));
    const created = await laravel().apply(await laravel().generateClass(
      { generator: "enum", name: "Audience", module: "", backing: "", inputs: {} }));
    const text = Buffer.from(await vscode.workspace.fs.readFile(created[0])).toString();
    assert.ok(text.includes("// from my template"), text);
    return `${templates.length} templates; ${path.relative(project, template.path)}`;
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
