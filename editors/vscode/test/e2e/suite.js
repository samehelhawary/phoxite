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

/** The refactorings that ask something first (what `activate` answers). */
const refactorings = () => vscode.extensions.getExtension("phoxite.phoxite").exports.refactorings;

/** The code action at `range` running client command `command` (and titled `title`). */
async function dialogAction(doc, range, command, title) {
  const wanted = (a) => a.command?.command === command && (!title || a.title === title);
  const actions = await until(command, () => vscode.commands.executeCommand("vscode.executeCodeActionProvider", doc.uri, range),
    (found) => (found ?? []).some(wanted));
  return actions.find(wanted);
}

const hoverText = (hovers) =>
  (hovers ?? []).flatMap((h) => h.contents.map((c) => (typeof c === "string" ? c : c.value))).join("\n");

const checks = [
  ["the extension activates", async () => {
    const ext = vscode.extensions.getExtension("phoxite.phoxite");
    assert.ok(ext, "phoxite.phoxite is not installed");
    await ext.activate();
    const commands = await vscode.commands.getCommands(true);
    for (const id of ["phoxite.restart", "phoxite.rescan", "laravel.routeActions", "phoxite.openLocation", "phoxite.status",
      "phoxite.server.laravel.routeActions", "phoxite.server.laravel.modelView"]) {
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
      (l) => (l ?? []).some((x) => x.command?.command === "phoxite.server.laravel.routeActions"));
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
  ["Twig: language, outline, a filter's hover, goto the parent template", async () => {
    await vscode.workspace.fs.writeFile(file("templates/base.html.twig"), Buffer.from("{% block body %}{% endblock %}\n"));
    await vscode.workspace.fs.writeFile(
      file("templates/page.html.twig"),
      Buffer.from("{% extends 'base.html.twig' %}\n{% block body %}{{ title|upper }}{% endblock %}\n"),
    );
    const doc = await open("templates/page.html.twig");
    assert.equal(doc.languageId, "twig");
    const symbols = await until("twig outline", () => vscode.commands.executeCommand("vscode.executeDocumentSymbolProvider", doc.uri),
      (s) => (s ?? []).length > 0, 30_000);
    assert.ok(symbols.some((s) => s.name === "body"), symbols.map((s) => s.name).join(", "));
    const hovers = await vscode.commands.executeCommand("vscode.executeHoverProvider", doc.uri, at(doc, "upper", 2));
    assert.match(hoverText(hovers), /upper/);
    const targets = await vscode.commands.executeCommand("vscode.executeDefinitionProvider", doc.uri, at(doc, "base.html.twig", 2));
    const target = (targets[0].targetUri ?? targets[0].uri).fsPath;
    assert.ok(target.endsWith(path.join("templates", "base.html.twig")), target);
    return `languageId twig, ${symbols.length} top-level symbols, hover: ${hoverText(hovers).split("\n").find((l) => l.includes("upper"))}`;
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
  ["Laravel: New Model with an enum, a foreign key's on delete, morphs and a relation through a model", async () => {
    const created = await laravel().apply(await laravel().newModel({
      name: "Ticket", module: "",
      fields: [
        { name: "status", ty: "enum", values: ["open", "closed"] },
        { name: "team_id", ty: "foreignUuid", onDelete: "cascade" },
        { name: "subject", ty: "morphs" },
      ],
      relations: [{ kind: "hasManyThrough", model: "App\\Models\\Post", through: "App\\Models\\User", name: "" }],
      indexes: [], timestamps: true, softDeletes: false, migration: true, factory: false, seeder: false,
      controller: "none", formRequests: false, resource: false, policy: false,
    }));
    const read = async (pattern) => {
      const uri = created.find((u) => pattern.test(u.fsPath));
      assert.ok(uri, `no ${pattern} among ${created.map((u) => path.basename(u.fsPath)).join(", ")}`);
      return Buffer.from(await vscode.workspace.fs.readFile(uri)).toString("utf8");
    };
    const migration = await read(/_create_tickets_table\.php$/);
    for (const line of [
      "$table->enum('status', ['open', 'closed']);",
      "$table->foreignUuid('team_id')->constrained()->cascadeOnDelete();",
      "$table->morphs('subject');",
    ]) assert.ok(migration.includes(line), `no ${line} in\n${migration}`);
    const model = await read(/Ticket\.php$/);
    assert.ok(model.includes("return $this->hasManyThrough(Post::class, User::class);"), model);
    assert.ok(model.includes("'subject_type',") && model.includes("'subject_id',"), model);
    return "the migration and the model have them";
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
      (l) => (l ?? []).some((x) => x.command?.command === "phoxite.server.laravel.runArtisan"));
    const lens = lenses.find((l) => l.command?.command === "phoxite.server.laravel.runArtisan");
    assert.deepEqual(lens.command.arguments, ["posts:publish"]);
    const tasks = await vscode.tasks.fetchTasks({ type: "artisan" });
    const task = tasks.find((t) => t.definition.command === "posts:publish");
    assert.ok(task, `no task in ${tasks.map((t) => t.name).join(", ")}`);
    assert.equal(task.execution.commandLine, "php artisan posts:publish");
    return `${commands.length} commands; lens "${lens.command.title}"; ${tasks.length} tasks`;
  }],
  ["Laravel: an Artisan command runs as it is set up, and Run Last runs it again", async () => {
    // `echo` stands in for PHP: what matters is the command line and where it runs.
    const settings = vscode.workspace.getConfiguration("phoxite");
    await settings.update("laravel.artisanCommand", "echo artisan", vscode.ConfigurationTarget.Workspace);
    const started = [];
    let ended = 0;
    const listener = vscode.Disposable.from(
      vscode.tasks.onDidStartTask((e) => started.push(e.execution.task)),
      vscode.tasks.onDidEndTask(() => ended++),
    );
    try {
      await open("app/Console/Commands/PublishScheduledPosts.php");
      await vscode.commands.executeCommand("phoxite.server.laravel.runArtisan", "posts:publish");
      // A task that still runs is not started a second time: VS Code asks first.
      await until("the task", async () => ended, (n) => n === 1, 20_000);
      await vscode.commands.executeCommand("phoxite.laravel.artisanLast");
      await until("the task again", async () => started.length, (n) => n === 2, 20_000);
    } finally {
      listener.dispose();
      await settings.update("laravel.artisanCommand", undefined, vscode.ConfigurationTarget.Workspace);
    }
    for (const task of started) {
      assert.equal(task.execution.commandLine, "echo artisan posts:publish");
      assert.equal(path.normalize(task.execution.options.cwd).toLowerCase(), path.normalize(project).toLowerCase());
    }
    return `${started.length} runs of "${started[0].execution.commandLine}"`;
  }],
  ["Laravel: Create translation key, the action and the edit", async () => {
    const uri = file("app/Translated.php");
    await vscode.workspace.fs.writeFile(uri, Buffer.from("<?php\nnamespace App;\n\nfunction translated(): string {\n    return __('messages.from_the_test');\n}\n"));
    const doc = await open("app/Translated.php");
    const pos = at(doc, "messages.from_the_test", 3);
    const actions = await until("the action", () => vscode.commands.executeCommand("vscode.executeCodeActionProvider", doc.uri, new vscode.Range(pos, pos)),
      (a) => (a ?? []).some((x) => x.command?.command === "phoxite.server.laravel.createTranslation"));
    const action = actions.find((a) => a.command?.command === "phoxite.server.laravel.createTranslation");
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
  ["Laravel: no database is read unless the setting names one", async () => {
    assert.equal(await laravel().refreshSchema(), null);
    const commands = await vscode.commands.getCommands(true);
    assert.ok(commands.includes("phoxite.laravel.refreshSchema"));
  }],
  ["Refactorings: the actions with a dialog are offered", async () => {
    await vscode.workspace.fs.writeFile(file("app/Shop/Base.php"), Buffer.from("<?php\n\nnamespace App\\Shop;\n\nclass Base\n{\n}\n"));
    await vscode.workspace.fs.writeFile(file("app/Shop/Shop.php"), Buffer.from(
      "<?php\n\nnamespace App\\Shop;\n\nclass Shop extends Base\n{\n    private int $count = 0;\n    private string $label = '';\n\n"
      + "    public function total(int $a, int $b): int\n    {\n        $sum = $a + $b;\n        return $sum * 2;\n    }\n\n"
      + "    public function run(): int\n    {\n        return $this->total(1, 2);\n    }\n}\n"));
    const doc = await open("app/Shop/Shop.php");
    const name = at(doc, "total(int");
    const signature = await dialogAction(doc, new vscode.Range(name, name), "phoxite.changeSignature");
    assert.equal(signature.command.arguments[0], doc.uri.toString());
    assert.equal(signature.command.arguments[1].signature.name, "Shop::total");
    const property = at(doc, "$count", 1);
    const getters = await dialogAction(doc, new vscode.Range(property, property), "phoxite.generateMembers", "Generate getters…");
    const statement = at(doc, "$sum = $a + $b;");
    const extract = await dialogAction(doc, new vscode.Range(statement, statement.translate(0, "$sum = $a + $b;".length)), "phoxite.extractMethod");
    return [signature, getters, extract].map((a) => a.title).join(" | ");
  }],
  ["Refactorings: the Change Signature form opens from the command", async () => {
    const doc = await open("app/Shop/Shop.php");
    const name = at(doc, "total(int");
    vscode.window.activeTextEditor.selection = new vscode.Selection(name, name);
    await vscode.commands.executeCommand("phoxite.refactor.changeSignature");
    const tabs = () => vscode.window.tabGroups.all.flatMap((g) => g.tabs).filter((t) => t.input instanceof vscode.TabInputWebview);
    const form = await until("the form", async () => tabs().find((t) => t.input.viewType.includes("phoxite.changeSignature")), Boolean, 20_000);
    const label = form.label;
    await vscode.window.tabGroups.close(form);
    return `tab: ${label}`;
  }],
  ["Refactorings: the answers are applied", async () => {
    const doc = await open("app/Shop/Shop.php");
    // Generate getters for one of the two properties.
    const property = at(doc, "$count", 1);
    const getters = await dialogAction(doc, new vscode.Range(property, property), "phoxite.generateMembers", "Generate getters…");
    const chooser = getters.command.arguments[1];
    assert.deepEqual(chooser.members.map((m) => m.id), ["count", "label"]);
    await refactorings().resolve("generate", doc.uri.toString(), { ...chooser, chosen: ["count"] });
    assert.ok(doc.getText().includes("function getCount()") && !doc.getText().includes("getLabel"), doc.getText());
    // Extract the sum as `add`.
    const statement = at(doc, "$sum = $a + $b;");
    const selected = new vscode.Range(statement, statement.translate(0, "$sum = $a + $b;".length));
    const extract = (await dialogAction(doc, selected, "phoxite.extractMethod")).command.arguments[1];
    await refactorings().resolve("extractMethod", doc.uri.toString(), { ...extract, change: { ...extract.defaults, name: "add" } });
    assert.ok(/function add\(int \$a, int \$b\)/.test(doc.getText()), doc.getText());
    // Change the signature: swap the parameters and rename the method.
    const name = at(doc, "total(int");
    const data = (await dialogAction(doc, new vscode.Range(name, name), "phoxite.changeSignature")).command.arguments[1];
    const change = { ...data.signature, functionName: "sum", parameters: [...data.signature.parameters].reverse() };
    await refactorings().resolve("changeSignature", doc.uri.toString(), { ...data, change });
    assert.ok(doc.getText().includes("function sum(int $b, int $a): int"), doc.getText());
    assert.ok(doc.getText().includes("$this->sum(2, 1)"), doc.getText());
    // Pull the method up to the parent class.
    const moved = at(doc, "sum(int");
    const position = { line: moved.line, character: moved.character };
    const ask = { uri: doc.uri.toString(), position, kind: "pullUp" };
    const info = await until("the parent", () => refactorings().request("hierarchyMembers", ask).catch(() => null), (i) => i?.targets.length);
    assert.equal(info.targets[0].qualifiedName, "App\\Shop\\Base", JSON.stringify(info));
    const result = await refactorings().request("hierarchy", { ...ask, target: info.targets[0].id, choices: [{ id: "method:sum", makeAbstract: false }] });
    assert.ok(result.edit, JSON.stringify(result.problems));
    await refactorings().apply(result.edit);
    const base = await vscode.workspace.openTextDocument(file("app/Shop/Base.php"));
    assert.ok(base.getText().includes("function sum(int $b, int $a): int"), base.getText());
    assert.ok(!doc.getText().includes("function sum("), doc.getText());
    await vscode.workspace.saveAll();
    return `getCount, add, sum(int $b, int $a) moved to Base; ${result.problems.length} problems`;
  }],
  // Last: it stops and starts the server the other checks use.
  ["Restart recovers from a server that didn't start", async () => {
    const config = vscode.workspace.getConfiguration("phoxite");
    const good = config.inspect("server.path").globalValue;
    await config.update("server.path", path.join(project, "no-such-server"), vscode.ConfigurationTarget.Global);
    await vscode.commands.executeCommand("phoxite.restart");
    assert.equal(laravel().isActive, false, "nothing is known about the project without a server");
    await assert.rejects(laravel().routes(), /isn't running/);
    // A second failed start, then the same command with the right path.
    await vscode.commands.executeCommand("phoxite.restart");
    await config.update("server.path", good, vscode.ConfigurationTarget.Global);
    await vscode.commands.executeCommand("phoxite.restart");
    await until("the Laravel project again", async () => laravel().isActive, (active) => active === true);
    const routes = await until("routes", () => laravel().routes().catch(() => []), (found) => found.length > 0);
    return `${routes.length} routes after the restart`;
  }],
];

/** The checks of a workspace with two folders, each a Laravel project (run.js makes it). */
const second = process.env.PHOXITE_E2E_SECOND;
const inFolder = (fsPath, root) => fsPath.toLowerCase().startsWith(root.toLowerCase() + path.sep);
const twoProjectChecks = [
  ["Two projects: a request stays with the project it was made for", async () => {
    await vscode.extensions.getExtension("phoxite.phoxite").activate();
    await until("both Laravel projects", async () => laravel().folders, (found) => found.length === 2);
    // An editor of the second project is active: a panel opened now is about that project…
    const model = vscode.Uri.file(path.join(second, "app/Models/Post.php"));
    await vscode.window.showTextDocument(await vscode.workspace.openTextDocument(model));
    const kept = laravel().in();
    assert.ok(inFolder(model.fsPath, vscode.Uri.parse(kept.folder).fsPath), kept.folder);
    assert.equal(laravel().of(model).folder, kept.folder, "the Explorer's Generate on a file of the project");
    // …and stays so when the first project gets the focus.
    await open("routes/web.php");
    assert.notEqual(laravel().folder, kept.folder);
    const view = await until("the model", () => kept.modelView("App\\Models\\Post").catch(() => undefined), Boolean);
    const created = await kept.apply(await kept.addColumns(view.class, [{ name: "subtitle", ty: "string", nullable: true }], false));
    assert.ok(created.length > 0 && created.every((uri) => inFolder(uri.fsPath, second)), created.map((u) => u.fsPath).join(", "));
    return `${path.relative(path.dirname(second), created[0].fsPath)}`;
  }],
  ["Two projects: the routes of each, and Artisan tasks in each folder", async () => {
    const counts = [];
    for (const folder of laravel().folders) {
      const root = vscode.Uri.parse(folder).fsPath;
      const routes = await until("routes", () => laravel().in(folder).routes().catch(() => []), (found) => found.length > 0);
      assert.ok(routes.every((route) => inFolder(route.path, root)), `routes outside ${root}`);
      counts.push(routes.length);
    }
    const tasks = await vscode.tasks.fetchTasks({ type: "artisan" });
    const scopes = new Set(tasks.map((task) => (typeof task.scope === "object" ? task.scope.name : "")));
    assert.deepEqual([...scopes].sort(), ["app", "shop"]);
    return `${counts.join(" + ")} routes; ${tasks.length} tasks in ${[...scopes].join(", ")}`;
  }],
];

/** The checks of the WordPress fixture (run.js opens it): a plugin and a theme on a stand-in API. */
const SHOP = "wp-content/plugins/shop";
const THEME = "wp-content/themes/shelf";
const provider = (command, doc, pos, ...rest) => vscode.commands.executeCommand(command, doc.uri, pos, ...rest);
const relative = (uri) => path.relative(project, uri.fsPath).split(path.sep).join("/");
const filled = (found) => (found ?? []).length > 0;
const labelOf = (item) => (typeof item.label === "string" ? item.label : item.label.label);

/** Where Go to Definition lands from `needle` (+ `offset`) of `rel`: `file:line`, lines from 1. */
async function goes(rel, needle, offset = 2) {
  const doc = await open(rel);
  const found = await until(`definition of ${needle}`, () => provider("vscode.executeDefinitionProvider", doc, at(doc, needle, offset)), filled);
  return found.map((target) => `${relative(target.uri ?? target.targetUri)}:${(target.range ?? target.targetRange).start.line + 1}`);
}

async function hoverAt(rel, needle, offset = 2) {
  const doc = await open(rel);
  const hovers = await until(`hover of ${needle}`, () => provider("vscode.executeHoverProvider", doc, at(doc, needle, offset)), filled);
  return hovers.flatMap((hover) => hover.contents).map((content) => content.value ?? content).join(" ");
}

/** The labels completion offers at `needle` (+ `offset`), once `wanted` is among them. */
async function completes(rel, needle, offset, wanted) {
  const doc = await open(rel);
  const list = await until(`completion at ${needle}`, () => provider("vscode.executeCompletionItemProvider", doc, at(doc, needle, offset)),
    (found) => (found?.items ?? []).some((item) => labelOf(item) === wanted));
  return list.items.map(labelOf);
}

async function usagesOf(rel, needle, offset, count) {
  const doc = await open(rel);
  const found = await until(`usages of ${needle}`, () => provider("vscode.executeReferenceProvider", doc, at(doc, needle, offset)),
    (usages) => (usages ?? []).length >= count);
  return found.map((usage) => `${relative(usage.uri)}:${usage.range.start.line + 1}`).sort();
}

/** The edits of a rename at `needle`, as `file:line` (nothing is applied); an empty list when it is refused. */
async function renames(rel, needle, offset, name) {
  const doc = await open(rel);
  const edit = await provider("vscode.executeDocumentRenameProvider", doc, at(doc, needle, offset), name).then((found) => found, () => undefined);
  const places = (edit?.entries() ?? []).flatMap(([uri, edits]) => edits.map((change) => `${relative(uri)}:${change.range.start.line + 1}`));
  return places.sort();
}

const wordpressChecks = [
  ["WordPress: the project is named", async () => {
    await open(`${SHOP}/shop.php`);
    await vscode.extensions.getExtension("phoxite.phoxite").activate();
    const names = await until("the frameworks", async () => laravel().frameworks, (found) => found.length > 0);
    assert.deepEqual([...names], ["WordPress"]);
    assert.equal(laravel().isActive, false, "not a Laravel project");
    return names.join(", ");
  }],
  ["WordPress: a hook goes to where it is fired", async () => {
    // Fired beside WordPress's folders, under a name put together, and by the plugin itself.
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "add_action('init'", 13), ["wp-settings.php:8"]);
    const pattern = await goes(`${SHOP}/shop.php`, "'save_post_book'", 3);
    assert.equal(pattern.length, 1);
    assert.ok(pattern[0].startsWith("wp-includes/api.php:"), pattern[0]);
    assert.deepEqual(await goes(`${SHOP}/includes/class-shop-catalog.php`, "has_action('shop_loaded'", 13), [`${SHOP}/shop.php:34`]);
    const hover = await hoverAt(`${SHOP}/shop.php`, "add_action('init'", 13);
    assert.ok(hover.includes("action") && hover.includes("wp-settings.php"), hover);
    return `${pattern[0]}; ${hover.replace(/\s+/g, " ").slice(0, 60)}`;
  }],
  ["WordPress: hooks complete, without the patterns", async () => {
    const labels = await completes(`${SHOP}/shop.php`, "add_action('init'", 12, "shop_loaded");
    for (const hook of ["init", "save_post", "the_title", "admin_init"]) assert.ok(labels.includes(hook), `${hook} in ${labels.join(", ")}`);
    assert.ok(!labels.some((label) => label.includes("*")), labels.join(", "));
    return `${labels.length} hooks`;
  }],
  ["WordPress: the usages of a hook, and its rename", async () => {
    const places = [`${SHOP}/includes/class-shop-catalog.php:7`, `${SHOP}/shop.php:14`, `${SHOP}/shop.php:34`];
    assert.deepEqual(await usagesOf(`${SHOP}/shop.php`, "do_action('shop_loaded'", 12, 3), places);
    assert.deepEqual(await renames(`${SHOP}/shop.php`, "do_action('shop_loaded'", 12, "shop_ready"), places);
    // Not a hook WordPress fires.
    assert.deepEqual(await renames(`${SHOP}/shop.php`, "add_action('init'", 13, "started"), []);
    return places.join(", ");
  }],
  ["WordPress: a callback is the function or the method it names", async () => {
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "'init', 'shop_register'", 10), [`${SHOP}/shop.php:16`]);
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "[Shop_Catalog::class, 'refresh']", 24), [`${SHOP}/includes/class-shop-catalog.php:12`]);
    // Under a key of an array: a REST route's, a block's.
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "'permission_callback' => 'shop_public'", 27), [`${SHOP}/shop.php:49`]);
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "'render_callback' => 'shop_price'", 22), [`${SHOP}/shop.php:44`]);
    // The function's usages count the strings.
    const usages = await usagesOf(`${SHOP}/shop.php`, "function shop_price", 12, 2);
    assert.ok(usages.includes(`${SHOP}/shop.php:20`) && usages.includes(`${SHOP}/shop.php:22`), usages.join(", "));
    return usages.join(", ");
  }],
  ["WordPress: a callback's parameters and the globals are typed", async () => {
    // What `save_post` passes a function registered by name; a closure's; a global in a method and in a template.
    const post = await completes(`${SHOP}/shop.php`, "$post->post_title);", 7, "post_title");
    assert.ok(post.includes("post_type"), post.join(", "));
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "$post->post_title);", 9), ["wp-includes/api.php:13"]);
    const wpdb = await completes(`${SHOP}/includes/class-shop-catalog.php`, "$wpdb->prepare", 7, "prepare");
    assert.ok(wpdb.includes("get_results") && wpdb.includes("prefix"), wpdb.join(", "));
    assert.deepEqual(await goes(`${THEME}/index.php`, "$post->post_title", 9), ["wp-includes/api.php:13"]);
    return `${post.join(", ")}; ${wpdb.join(", ")}`;
  }],
  ["WordPress: the names of the registries go to their registration", async () => {
    const catalog = `${SHOP}/includes/class-shop-catalog.php`;
    const functions = `${THEME}/functions.php`;
    const found = {
      "post type in a list": await goes(catalog, "'post_type' => ['book']", 18),
      "post type of is_singular": await goes(`${THEME}/index.php`, "is_singular(['book'])", 15),
      taxonomy: await goes(`${THEME}/index.php`, "get_the_terms($post, 'genre')", 23),
      shortcode: await goes(catalog, "shortcode_exists('price')", 19),
      option: await goes(`${SHOP}/shop.php`, "get_option('shop_currency')", 13),
      block: await goes(`${SHOP}/shop.php`, "has_block('shop/price')", 12),
      script: await goes(functions, "wp_enqueue_script('shelf-app')", 20),
      "script as a dependency": await goes(functions, "['shelf-app']", 3),
      "image size": await goes(`${THEME}/index.php`, "the_post_thumbnail('card')", 21),
      "menu location": await goes(`${THEME}/index.php`, "'theme_location' => 'primary'", 22),
    };
    assert.deepEqual(found, {
      "post type in a list": [`${SHOP}/shop.php:18`],
      "post type of is_singular": [`${SHOP}/shop.php:18`],
      taxonomy: [`${SHOP}/shop.php:19`],
      shortcode: [`${SHOP}/shop.php:20`],
      option: [`${SHOP}/shop.php:21`],
      block: [`${SHOP}/shop.php:22`],
      script: [`${functions}:9`],
      "script as a dependency": [`${functions}:9`],
      "image size": [`${functions}:7`],
      "menu location": [`${functions}:8`],
    });
    const hover = await hoverAt(catalog, "'post_type' => ['book']", 18);
    assert.ok(hover.includes("post type") && hover.includes("Registered in"), hover);
    // Completion offers the kind asked for, and a rename changes every call naming it.
    const options = await completes(`${SHOP}/shop.php`, "get_option('shop_currency')", 12, "shop_currency");
    assert.ok(options.includes("shop_last_saved") && !options.includes("book"), options.join(", "));
    const renamed = await renames(`${SHOP}/shop.php`, "register_post_type('book'", 20, "novel");
    assert.deepEqual(renamed, [`${catalog}:28`, `${SHOP}/shop.php:18`, `${SHOP}/shop.php:19`, `${THEME}/index.php:3`]);
    return `${Object.keys(found).length} kinds; rename in ${renamed.length} places`;
  }],
  ["WordPress: the files a theme names", async () => {
    const index = `${THEME}/index.php`;
    assert.deepEqual(await goes(index, "'template-parts/content', 'book'", 10), [`${THEME}/template-parts/content-book.php:1`]);
    assert.deepEqual(await goes(index, "get_template_part('template-parts/content');", 25), [`${THEME}/template-parts/content.php:1`]);
    assert.deepEqual(await goes(index, "get_header('shop')", 13), [`${THEME}/header-shop.php:1`]);
    assert.deepEqual(await goes(`${THEME}/functions.php`, "get_theme_file_uri('/assets/app.js'))", 22), [`${THEME}/assets/app.js:1`]);
    assert.deepEqual(await goes(`${THEME}/functions.php`, "'template-parts/content.php']", 5), [`${THEME}/template-parts/content.php:1`]);
    // Right after the quote: nothing typed narrows the list.
    const parts = (await completes(index, "get_template_part('template-parts/content');", 19, "template-parts/content-book"))
      .filter((label) => /^(header|footer|index|functions|template-parts)/.test(label));
    assert.ok(parts.includes("header-shop") && parts.includes("footer"), parts.join(", "));
    return parts.join(", ");
  }],
  ["WordPress: a REST route by its path", async () => {
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "rest_url('shop/v1/books')", 14), [`${SHOP}/shop.php:23`]);
    const hover = await hoverAt(`${SHOP}/shop.php`, "rest_url('shop/v1/books')", 14);
    assert.ok(hover.includes("REST route") && hover.includes("shop/v1/books"), hover);
    const routes = await completes(`${SHOP}/shop.php`, "rest_url('shop/v1/books')", 10, "shop/v1/books");
    // Usages from the route's own string, the script included; a rename is of the last part.
    const places = [`${SHOP}/assets/shop.js:6`, `${SHOP}/shop.php:23`, `${SHOP}/shop.php:59`];
    assert.deepEqual(await usagesOf(`${SHOP}/shop.php`, "'shop/v1', '/books'", 14, 3), places);
    assert.deepEqual(await renames(`${SHOP}/shop.php`, "rest_url('shop/v1/books')", 14, "novels"), places);
    return `${routes.length} routes; ${places.join(", ")}`;
  }],
  ["WordPress: a script names a block, a REST route and an AJAX action", async () => {
    // The server asks for the scripts once it knows the project: no selector lists them.
    const script = `${SHOP}/assets/shop.js`;
    assert.deepEqual(await goes(script, "'shop/stock'", 3), [`${SHOP}/blocks/stock/block.json:3`]);
    assert.deepEqual(await goes(script, "'/shop/v1/books'", 3), [`${SHOP}/shop.php:23`]);
    assert.deepEqual(await goes(script, "'shop_add'", 3), [`${SHOP}/shop.php:54`]);
    const hover = await hoverAt(script, "'shop_add'", 3);
    assert.ok(hover.includes("AJAX action") && hover.includes("shop.php"), hover);
    const actions = await completes(script, "action: 'shop_add'", 9, "shop_add");
    const routes = await completes(script, "path: '/shop/v1/books'", 7, "/shop/v1/books");
    // A rename of the action from the script: the hook in PHP and the script.
    const renamed = await renames(script, "'shop_add'", 3, "shop_put");
    assert.deepEqual(renamed, [`${script}:4`, `${SHOP}/shop.php:54`]);
    return `${actions.join(", ")}; ${routes.join(", ")}; rename in ${renamed.join(", ")}`;
  }],
  ["WordPress: a block.json declares a block", async () => {
    const json = `${SHOP}/blocks/stock/block.json`;
    assert.deepEqual(await goes(`${SHOP}/shop.php`, "has_block('shop/stock')", 12), [`${json}:3`]);
    const places = [`${SHOP}/assets/shop.js:2`, `${json}:3`, `${SHOP}/shop.php:59`];
    assert.deepEqual(await usagesOf(json, "shop/stock", 3, 3), places);
    // Renamed from PHP: the file and the script change with the call.
    assert.deepEqual(await renames(`${SHOP}/shop.php`, "has_block('shop/stock')", 12, "shop/in-stock"), places);
    const blocks = await completes(`${SHOP}/shop.php`, "has_block('shop/stock')", 11, "shop/stock");
    assert.ok(blocks.includes("shop/price"), blocks.join(", "));
    return places.join(", ");
  }],
  ["WordPress: a hook nothing fires is reported when the inspection is on", async () => {
    const doc = await open(`${SHOP}/shop.php`);
    const hooks = () => vscode.languages.getDiagnostics(doc.uri).filter((problem) => String(problem.code?.value ?? problem.code) === "PhpUndefinedHook");
    const edit = new vscode.WorkspaceEdit();
    edit.insert(doc.uri, doc.lineAt(doc.lineCount - 1).range.end, "\nadd_action('shop_nothing_fires', 'shop_add');\n");
    await vscode.workspace.applyEdit(edit);
    const config = vscode.workspace.getConfiguration("phoxite");
    try {
      // Off by default: the file is inspected and says nothing of the hook.
      await until("the callback's usage", () => provider("vscode.executeDefinitionProvider", doc, at(doc, "'shop_nothing_fires', 'shop_add'", 24)), filled);
      assert.deepEqual(hooks(), []);
      await config.update("inspections", { PhpUndefinedHook: { enabled: true } }, vscode.ConfigurationTarget.Global);
      const found = await until("the hook's problem", async () => hooks(), filled);
      // Only that one: every other hook of the file is fired somewhere.
      assert.deepEqual(found.map((problem) => problem.message), ["Hook 'shop_nothing_fires' is not fired anywhere"]);
      return found[0].message;
    } finally {
      await config.update("inspections", undefined, vscode.ConfigurationTarget.Global);
      await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    }
  }],
];

exports.run = async function run() {
  const results = [];
  const byMode = { "two-projects": twoProjectChecks, wordpress: wordpressChecks };
  const chosen = byMode[process.env.PHOXITE_E2E_MODE] ?? checks;
  for (const [name, check] of chosen) {
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
