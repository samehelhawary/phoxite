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
  ["Callable strings: a function's and a method's name in a string are names", async () => {
    const CALLABLES = "app/Support/callables.php";
    const source = [
      "<?php",
      "",
      "namespace App\\Support;",
      "",
      "function shorten(string $text): string",
      "{",
      "    return substr($text, 0, 10);",
      "}",
      "",
      "class Shortener",
      "{",
      "    public static function cut(string $text): string",
      "    {",
      "        return shorten($text);",
      "    }",
      "}",
      "",
      "/** @param list<string> $names */",
      "function tidy(array $names): array",
      "{",
      "    $short = array_map('App\\Support\\shorten', $names);",
      "",
      "    return array_map([Shortener::class, 'cut'], $short);",
      "}",
      "",
    ].join("\n");
    await vscode.workspace.fs.writeFile(file(CALLABLES), Buffer.from(source));
    // The string goes to the function, the array's to the method.
    assert.deepEqual(await goes(CALLABLES, "'App\\Support\\shorten'", 16), [`${CALLABLES}:5`]);
    assert.deepEqual(await goes(CALLABLES, "'cut']", 2), [`${CALLABLES}:12`]);
    // The string is a usage of the function, and is renamed with it.
    const sorted = (found) => [...found].sort();
    const usages = sorted([`${CALLABLES}:5`, `${CALLABLES}:14`, `${CALLABLES}:21`]);
    assert.deepEqual(sorted(await usagesOf(CALLABLES, "function shorten", 10, 3)), usages);
    assert.deepEqual(sorted(await renames(CALLABLES, "function shorten", 10, "abbreviate")), usages);
    const method = sorted([`${CALLABLES}:12`, `${CALLABLES}:23`]);
    assert.deepEqual(sorted(await usagesOf(CALLABLES, "function cut", 10, 2)), method);
    assert.deepEqual(sorted(await renames(CALLABLES, "'cut']", 2, "trim")), method);
    return `${usages.length} usages of a function, ${method.length} of a method, the strings among them`;
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

/** The checks of the Drupal fixture (run.js opens it): a module and a theme on a stand-in API. */
const MOD = "modules/shop";
const CONTROLLER = `${MOD}/src/Controller/BookController.php`;
const drupalChecks = [
  ["Drupal: the project is named, and a module's file is PHP", async () => {
    const doc = await open(`${MOD}/shop.module`);
    assert.equal(doc.languageId, "php", "`.module` is associated with PHP");
    assert.equal((await open("themes/shelf/shelf.theme")).languageId, "php");
    await vscode.extensions.getExtension("phoxite.phoxite").activate();
    const names = await until("the frameworks", async () => laravel().frameworks, (found) => found.length > 0);
    assert.deepEqual([...names], ["Drupal"]);
    return names.join(", ");
  }],
  ["Drupal: the names of the YAML files go to their keys", async () => {
    const found = {
      service: await goes(CONTROLLER, "service('shop.catalog')", 12),
      route: await goes(CONTROLLER, "'Books', 'shop.books'", 12),
      "route in a .module": await goes(`${MOD}/shop.module`, "setRedirect('shop.books')", 15),
      permission: await goes(CONTROLLER, "hasPermission('administer shop')", 17),
      library: await goes(CONTROLLER, "[] = 'shop/base'", 7),
      "library in a .theme": await goes("themes/shelf/shelf.theme", "[] = 'shop/cart'", 7),
      configuration: await goes(CONTROLLER, "config('shop.settings')", 10),
    };
    assert.deepEqual(found, {
      service: [`${MOD}/shop.services.yml:2`],
      route: [`${MOD}/shop.routing.yml:1`],
      "route in a .module": [`${MOD}/shop.routing.yml:1`],
      permission: [`${MOD}/shop.permissions.yml:1`],
      library: [`${MOD}/shop.libraries.yml:6`],
      "library in a .theme": [`${MOD}/shop.libraries.yml:1`],
      configuration: [`${MOD}/config/install/shop.settings.yml:1`],
    });
    const hover = await hoverAt(CONTROLLER, "'Books', 'shop.books'", 12);
    assert.ok(hover.includes("route") && hover.includes("/books"), hover);
    const routes = await completes(CONTROLLER, "redirect('shop.books')", 10, "shop.book");
    assert.ok(routes.includes("shop.books") && !routes.includes("shop.catalog"), routes.join(", "));
    const permissions = await completes(CONTROLLER, "hasPermission('administer shop')", 15, "access shop");
    return `${Object.keys(found).length} places; ${routes.length} routes, ${permissions.length} permissions`;
  }],
  ["Drupal: usages and renames count the YAML files", async () => {
    const service = [`${MOD}/shop.services.yml:2`, `${MOD}/shop.services.yml:8`, `${CONTROLLER}:21`];
    assert.deepEqual(await usagesOf(CONTROLLER, "service('shop.catalog')", 12, 3), service);
    assert.deepEqual(await renames(CONTROLLER, "service('shop.catalog')", 12, "shop.library"), service);
    const route = [
      `${MOD}/shop.links.menu.yml:3`, `${MOD}/shop.module:31`, `${MOD}/shop.routing.yml:1`, `${CONTROLLER}:25`, `${CONTROLLER}:35`,
    ];
    assert.deepEqual(await renames(CONTROLLER, "redirect('shop.books')", 12, "shop.list"), route);
    // A permission, written with a space; a library, by the part after the module's name.
    const permission = await renames(CONTROLLER, "hasPermission('administer shop')", 17, "manage shop");
    assert.deepEqual(permission, [`${MOD}/shop.permissions.yml:1`, `${MOD}/shop.routing.yml:6`, `${CONTROLLER}:24`]);
    const library = await renames(CONTROLLER, "[] = 'shop/base'", 13, "reset");
    assert.deepEqual(library, [
      `${MOD}/shop.info.yml:5`, `${MOD}/shop.libraries.yml:5`, `${MOD}/shop.libraries.yml:6`, `${CONTROLLER}:23`, "themes/shelf/shelf.info.yml:6",
    ]);
    // Not a service of Drupal's own files.
    assert.deepEqual(await renames(`${MOD}/src/Hook/ShopHooks.php`, "moduleExists('shop')", 15, "store"), []);
    return `${route.length} places for a route, ${library.length} for a library`;
  }],
  ["Drupal: entity types, plugins and theme hooks come from PHP", async () => {
    assert.deepEqual(await goes(`${MOD}/src/Catalog.php`, "getStorage('shop_book')", 14), [`${MOD}/src/Entity/Book.php:9`]);
    assert.deepEqual(await goes(CONTROLLER, "getStorage('shop_shelf')", 14), [`${MOD}/src/Entity/Shelf.php:11`]);
    assert.deepEqual(await goes(CONTROLLER, "createInstance('shop_cart')", 18), [`${MOD}/src/Plugin/Block/CartBlock.php:8`]);
    // A theme hook: the key of `hook_theme()`, and its template.
    const theme = await goes(CONTROLLER, "'#theme' => 'shop_book'", 15);
    assert.deepEqual(theme.sort(), [`${MOD}/shop.module:9`, `${MOD}/templates/shop-book.html.twig:1`]);
    assert.deepEqual(await goes(`${MOD}/shop.module`, "['#theme'] = 'shop_price_tag'", 20), [`${MOD}/shop.module:10`]);
    const types = await completes(CONTROLLER, "getStorage('shop_shelf')", 12, "shop_book");
    assert.ok(types.includes("shop_shelf"), types.join(", "));
    const hover = await hoverAt(CONTROLLER, "getStorage('shop_shelf')", 14);
    assert.ok(hover.includes("entity type") && hover.includes("Shelf"), hover);
    return theme.join(", ");
  }],
  ["Drupal: a hook is its api function", async () => {
    const hooks = `${MOD}/src/Hook/ShopHooks.php`;
    // By a placeholder, an alter hook, and one fired by name.
    assert.deepEqual(await goes(hooks, "Hook('shop_book_presave')", 12), ["core/core.api.php:27"]);
    assert.deepEqual(await goes(`${MOD}/shop.module`, "alter('form'", 8), ["core/core.api.php:37"]);
    assert.deepEqual(await goes(hooks, "invokeAll('entity_presave'", 16), ["core/core.api.php:17"]);
    const offered = await completes(hooks, "invokeAll('entity_presave'", 11, "cron");
    assert.ok(offered.includes("form_alter") && offered.includes("entity_presave"), offered.join(", "));
    // The usages count the function implementing it.
    const usages = await usagesOf(hooks, "invokeAll('entity_presave'", 16, 2);
    assert.ok(usages.includes(`${MOD}/shop.module:17`), usages.join(", "));
    const hover = await hoverAt(`${MOD}/shop.module`, "alter('form'", 8);
    assert.ok(hover.includes("hook_form_alter"), hover);
    return usages.join(", ");
  }],
  ["Drupal: services, storages and hook parameters are typed", async () => {
    // A service by its id; the storage an entity type names and what it loads; a storage nothing names.
    assert.deepEqual(await goes(CONTROLLER, "$catalog->books()", 12), [`${MOD}/src/Catalog.php:15`]);
    assert.deepEqual(await goes(`${MOD}/src/Catalog.php`, "$book->title !== ''", 8), [`${MOD}/src/Entity/Book.php:12`]);
    assert.deepEqual(await goes(CONTROLLER, "$shelf->room", 9), [`${MOD}/src/Entity/Shelf.php:17`]);
    // A function implementing a hook, in a `.module` and a `.theme`, and a method with the attribute.
    const state = await completes(`${MOD}/shop.module`, "$form_state->setRedirect", 13, "setRedirect");
    assert.ok(state.includes("getValue"), state.join(", "));
    const themed = await completes("themes/shelf/shelf.theme", "$form_state->getValue", 13, "getValue");
    const entity = await completes(`${MOD}/src/Hook/ShopHooks.php`, "$entity->label()", 9, "label");
    assert.ok(entity.includes("id"), entity.join(", "));
    return `${state.join(", ")}; ${themed.length} in the theme; ${entity.join(", ")}`;
  }],
  ["Drupal: a template includes a module's by its namespace", async () => {
    const found = await goes("themes/shelf/templates/page.html.twig", "@shop/shop-book.html.twig", 8);
    assert.deepEqual(found, [`${MOD}/templates/shop-book.html.twig:1`]);
    // A library's usages count the template attaching it.
    const usages = await usagesOf("themes/shelf/shelf.theme", "[] = 'shop/cart'", 9, 3);
    assert.ok(usages.includes(`${MOD}/templates/shop-book.html.twig:2`), usages.join(", "));
    return usages.join(", ");
  }],
  ["Drupal: a YAML file is answered in", async () => {
    // The server asks for the YAML files once it knows the project: no selector lists them.
    const routing = `${MOD}/shop.routing.yml`;
    assert.deepEqual(await goes(routing, "BookController::list", 17), [`${CONTROLLER}:19`]);
    assert.deepEqual(await goes(routing, "administer shop+", 3), [`${MOD}/shop.permissions.yml:1`]);
    assert.deepEqual(await goes(`${MOD}/shop.services.yml`, "'@shop.prices'", 4), [`${MOD}/shop.services.yml:5`]);
    assert.deepEqual(await goes(`${MOD}/shop.links.menu.yml`, "route_name: shop.books", 14), [`${MOD}/shop.routing.yml:1`]);
    assert.deepEqual(await goes(`${MOD}/shop.info.yml`, "- shop/base", 5), [`${MOD}/shop.libraries.yml:6`]);
    const hover = await hoverAt(`${MOD}/shop.services.yml`, "'@shop.prices'", 4);
    assert.ok(hover.includes("service") && hover.includes("Prices"), hover);
    // From the key declaring a route: its usages, and its rename.
    const places = [`${MOD}/shop.links.menu.yml:3`, `${MOD}/shop.module:31`, `${MOD}/shop.routing.yml:1`, `${CONTROLLER}:25`, `${CONTROLLER}:35`];
    assert.deepEqual(await usagesOf(routing, "shop.books:", 3, 5), places);
    assert.deepEqual(await renames(routing, "shop.books:", 3, "shop.list"), places);
    const permissions = await completes(routing, "administer shop+", 3, "access shop");
    const routes = await completes(`${MOD}/shop.links.menu.yml`, "route_name: shop.books", 13, "shop.book");
    return `${places.length} usages of a route; ${permissions.length} permissions, ${routes.length} routes offered`;
  }],
  ["Drupal: configuration keys, an implementation's hook, a theme hook's functions", async () => {
    const settings = `${MOD}/config/install/shop.settings.yml`;
    assert.deepEqual(await goes(CONTROLLER, "->get('currency')", 9), [`${settings}:1`]);
    assert.deepEqual(await goes(`${MOD}/shop.module`, "->get('mail.from')", 11), [`${settings}:3`]);
    const keys = await completes(CONTROLLER, "->get('currency')", 7, "currency");
    assert.ok(keys.includes("mail.from"), keys.join(", "));
    // The name of a function implementing a hook goes to the hook.
    assert.deepEqual(await goes(`${MOD}/shop.module`, "function shop_entity_presave", 20), ["core/core.api.php:17"]);
    assert.deepEqual(await goes("themes/shelf/shelf.theme", "function shelf_form_alter", 18), ["core/core.api.php:37"]);
    // The function preparing a theme hook's variables is a usage of it.
    const usages = await usagesOf(CONTROLLER, "'#theme' => 'shop_book'", 15, 3);
    assert.ok(usages.includes(`${MOD}/shop.module:38`), usages.join(", "));
    return `${keys.join(", ")}; ${usages.length} usages of the theme hook`;
  }],
  ["Drupal: a template knows its libraries and its variables", async () => {
    const template = `${MOD}/templates/shop-book.html.twig`;
    assert.deepEqual(await goes(template, "\"shop/cart\"", 4), [`${MOD}/shop.libraries.yml:1`]);
    const libraries = await completes(template, "\"shop/cart\"", 1, "shop/base");
    // What `hook_theme()` gives the template.
    const variables = await completes(template, "{{ book }}", 3, "book");
    assert.ok(variables.includes("attributes"), variables.join(", "));
    return `${libraries.join(", ")}; ${variables.filter((name) => ["book", "attributes"].includes(name)).join(", ")}`;
  }],
  ["Drupal: a template's variable has its type, and a YAML file completes classes", async () => {
    // What the preprocess function assigns the variable: its members follow the dot.
    const doc = await open(`${MOD}/templates/shop-book.html.twig`);
    const edit = new vscode.WorkspaceEdit();
    edit.insert(doc.uri, new vscode.Position(0, 0), "{{ book. }}" + String.fromCharCode(10));
    await vscode.workspace.applyEdit(edit);
    let members;
    try {
      const list = await until("members", () => provider("vscode.executeCompletionItemProvider", doc, new vscode.Position(0, 8)),
        (found) => (found?.items ?? []).some((item) => labelOf(item) === "title"));
      members = list.items.map(labelOf);
    } finally {
      await vscode.commands.executeCommand("workbench.action.revertAndCloseActiveEditor");
    }
    // After `class:`, the namespaces and classes below what is written.
    const classes = await completes(`${MOD}/shop.services.yml`, "class: Drupal\\shop\\Catalog", 19, "Drupal\\shop\\Prices");
    assert.ok(classes.includes("Drupal\\shop\\Entity"), classes.join(", "));
    return `${members.filter((name) => ["title", "label", "id"].includes(name)).join(", ")}; ${classes.length} names`;
  }],
];

/** The checks of the CodeIgniter fixture (run.js opens it): an application on a stand-in API. */
const BOOKS = "app/Controllers/Books.php";
const LIST = "app/Views/books/list.php";
const codeigniterChecks = [
  ["CodeIgniter: the project is named", async () => {
    await open(BOOKS);
    await vscode.extensions.getExtension("phoxite.phoxite").activate();
    const names = await until("the frameworks", async () => laravel().frameworks, (found) => found.length > 0);
    assert.deepEqual([...names], ["CodeIgniter"]);
    return names.join(", ");
  }],
  ["CodeIgniter: names go to what they name", async () => {
    const found = {
      view: await goes(BOOKS, "view('books/list'", 8),
      helper: await goes(BOOKS, "helper(['shop', 'url'])", 10),
      "helper of the framework": await goes(BOOKS, "helper(['shop', 'url'])", 18),
      "language line": await goes(BOOKS, "lang('Shop.cart.empty')", 8),
      route: await goes(BOOKS, "route('books.list')", 9),
      filter: await goes("app/Config/Routes.php", "'filter' => 'auth'", 13),
      "route's handler": await goes("app/Config/Routes.php", "'Books::show/$1'", 9),
      model: await goes(BOOKS, "model('BookModel')", 9),
      configuration: await goes(BOOKS, "config('Shop')", 9),
      service: await goes(BOOKS, "service('catalog')", 11),
    };
    assert.deepEqual(found, {
      view: [`${LIST}:1`],
      helper: ["app/Helpers/shop_helper.php:1"],
      "helper of the framework": ["system/Helpers/url_helper.php:1"],
      "language line": ["app/Language/en/Shop.php:6"],
      route: ["app/Config/Routes.php:6"],
      filter: ["app/Config/Filters.php:11"],
      "route's handler": [`${BOOKS}:26`],
      model: ["app/Models/BookModel.php:7"],
      configuration: ["app/Config/Shop.php:5"],
      service: ["app/Config/Services.php:10"],
    });
    const hover = await hoverAt(BOOKS, "lang('Shop.cart.empty')", 8);
    assert.ok(hover.includes("language line") && hover.includes("Nothing yet"), hover);
    return `${Object.keys(found).length} kinds`;
  }],
  ["CodeIgniter: names complete, and a route and a filter are renamed", async () => {
    const lines = await completes(BOOKS, "lang('Shop.cart.empty')", 6, "Shop.title");
    assert.ok(lines.includes("Validation.required"), lines.join(", "));
    const views = await completes(BOOKS, "view('books/list'", 6, "layout");
    const services = await completes(BOOKS, "service('catalog')", 9, "request");
    assert.ok(services.includes("catalog"), services.join(", "));
    const route = ["app/Config/Routes.php:6", `${BOOKS}:28`, "app/Filters/Auth.php:13"];
    assert.deepEqual(await usagesOf(BOOKS, "route('books.list')", 9, 3), route);
    assert.deepEqual(await renames(BOOKS, "route('books.list')", 9, "books.all"), route);
    const filter = ["app/Config/Filters.php:11", "app/Config/Filters.php:16", "app/Config/Routes.php:6"];
    assert.deepEqual(await renames("app/Config/Routes.php", "'filter' => 'auth'", 13, "member"), filter);
    // Not a view, whose name is a file's.
    assert.deepEqual(await renames(BOOKS, "view('books/list'", 8, "books/all"), []);
    return `${lines.length} lines, ${views.length} views, ${services.length} services`;
  }],
  ["CodeIgniter: events, cells, rules, tables and columns are names", async () => {
    const ORDERS = "app/Controllers/Orders.php";
    const MIGRATION = "app/Database/Migrations/2024-01-01-000000_CreateBooks.php";
    const found = {
      event: await goes("app/Config/Events.php", "'order_placed'", 8),
      "view cell": await goes(ORDERS, "'Card::render'", 9),
      rule: await goes(ORDERS, "'required|valid_email'", 14),
      "rule group": await goes(ORDERS, "'order')", 3),
      table: await goes(ORDERS, "table('books')", 9),
      column: await goes(ORDERS, "where('title'", 9),
      "column along the chain": await goes(ORDERS, "orderBy('price')", 11),
      "column of a model": await goes("app/Models/ShelfModel.php", "['title', 'price']", 13),
      "route by its path": await goes(ORDERS, "route_to('orders')", 12),
    };
    assert.deepEqual(found, {
      event: [`${ORDERS}:18`],
      "view cell": ["app/Cells/Card.php:7"],
      rule: ["system/More.php:59"],
      "rule group": ["app/Config/Validation.php:14"],
      table: [`${MIGRATION}:16`],
      column: [`${MIGRATION}:13`],
      "column along the chain": [`${MIGRATION}:14`],
      "column of a model": [`${MIGRATION}:14`],
      "route by its path": ["app/Config/Routes.php:8"],
    });
    const rules = await completes(ORDERS, "'required|valid_email'", 10, "valid_email");
    const columns = await completes(ORDERS, "where('title'", 7, "price");
    assert.deepEqual(await renames("app/Config/Events.php", "'order_placed'", 8, "order_made"), [
      "app/Config/Events.php:5",
      `${ORDERS}:18`,
    ]);
    return `${Object.keys(found).length} kinds, ${rules.length} rules, ${columns.length} columns`;
  }],
  ["CodeIgniter: a name that names nothing is reported", async () => {
    const ORDERS = "app/Controllers/Orders.php";
    const document = await open(ORDERS);
    const edit = new vscode.WorkspaceEdit();
    const at = document.getText().indexOf("'required|valid_email'");
    edit.replace(document.uri, new vscode.Range(document.positionAt(at), document.positionAt(at + 22)), "'required|nope'");
    await vscode.workspace.applyEdit(edit);
    const problems = await until(
      "the unknown rule",
      async () => vscode.languages.getDiagnostics(document.uri).map((problem) => problem.message),
      (found) => found.some((message) => message.includes("nope")),
    );
    await vscode.commands.executeCommand("workbench.action.files.revert");
    assert.deepEqual(problems, ["Unknown validation rule 'nope'"]);
    return problems.join("; ");
  }],
  ["CodeIgniter: the helper functions and a view's variables are typed", async () => {
    assert.deepEqual(await goes(BOOKS, "$model->titles()", 10), ["app/Models/BookModel.php:12"]);
    assert.deepEqual(await goes(BOOKS, "$shop->currency", 8), ["app/Config/Shop.php:7"]);
    // In the view: what the controller passes, and the object rendering it.
    assert.deepEqual(await goes(LIST, "$catalog->count()", 12), ["app/Libraries/Catalog.php:7"]);
    assert.deepEqual(await goes(LIST, "extend('layout')", 9), ["app/Views/layout.php:1"]);
    const members = await completes(LIST, "$catalog->count()", 10, "count");
    const problems = vscode.languages.getDiagnostics(file(LIST)).map((problem) => problem.message);
    assert.deepEqual(problems, [], "a view's variables aren't undefined");
    return `${members.join(", ")}; no problems in the view`;
  }],
];

/** The checks of the Symfony fixture (run.js opens it): an application on a stand-in API. */
const BLOG = "src/Controller/BlogController.php";
const SHOW = "templates/blog/show.html.twig";
const symfonyChecks = [
  ["Symfony: the project is named", async () => {
    await open(BLOG);
    await vscode.extensions.getExtension("phoxite.phoxite").activate();
    const names = await until("the frameworks", async () => laravel().frameworks, (found) => found.length > 0);
    assert.deepEqual([...names], ["Symfony"]);
    return names.join(", ");
  }],
  ["Symfony: names in PHP go to the YAML and the template declaring them", async () => {
    const found = {
      route: await goes(BLOG, "redirectToRoute('home')", 18),
      "route with a controller": await goes(BLOG, "generateUrl('blog_show')", 15),
      template: await goes(BLOG, "render('blog/show.html.twig'", 10),
      service: await goes(BLOG, "get('mailer.default')", 7),
      translation: await goes(BLOG, "trans('blog.title')", 9),
    };
    assert.deepEqual(found, {
      route: ["config/routes.yaml:1"],
      "route with a controller": ["config/routes.yaml:3"],
      template: [`${SHOW}:1`],
      service: ["config/services.yaml:7"],
      translation: ["translations/messages.en.yaml:2"],
    });
    const hover = await hoverAt(BLOG, "generateUrl('blog_show')", 15);
    assert.ok(hover.includes("/blog/{slug}"), hover);
    return `${Object.keys(found).length} kinds; ${hover.split("\n")[0]}`;
  }],
  ["Symfony: names complete, and a service id types what the container gives", async () => {
    const routes = await completes(BLOG, "redirectToRoute('home')", 17, "blog_show");
    const templates = await completes(BLOG, "render('blog/show.html.twig'", 8, "base.html.twig");
    const services = await completes(BLOG, "get('mailer.default')", 5, "mailer");
    const keys = await completes(BLOG, "trans('blog.title')", 7, "blog.empty");
    // `$container->get('mailer.default')` is the service's class.
    assert.deepEqual(await goes(BLOG, "$mailer->send(", 10), ["src/Service/Mailer.php:7"]);
    return `${routes.length} routes, ${templates.length} templates, ${services.length} services, ${keys.length} keys`;
  }],
  ["Symfony: Twig names routes and templates, and a route is renamed everywhere", async () => {
    assert.deepEqual(await goes(SHOW, "path('blog_show'", 8), ["config/routes.yaml:3"]);
    assert.deepEqual(await goes(SHOW, "extends 'base.html.twig'", 12), ["templates/base.html.twig:1"]);
    const usages = await usagesOf(BLOG, "redirectToRoute('home')", 18, 3);
    assert.deepEqual(usages, ["config/routes.yaml:1", `${BLOG}:15`, "templates/base.html.twig:1"]);
    assert.deepEqual(await renames(BLOG, "redirectToRoute('home')", 18, "front"), usages);
    const members = await completes(SHOW, "{{ title }}", 3, "title");
    assert.ok(members.includes("link"), members.join(", "));
    return `${usages.length} usages of a route; a template's variables: ${members.filter((name) => name === "title" || name === "link").join(", ")}`;
  }],
  ["Symfony: a template that is not there is reported", async () => {
    const document = await open(BLOG);
    const edit = new vscode.WorkspaceEdit();
    const at = document.getText().indexOf("'blog/show.html.twig'");
    edit.replace(document.uri, new vscode.Range(document.positionAt(at), document.positionAt(at + 21)), "'blog/nope.html.twig'");
    await vscode.workspace.applyEdit(edit);
    const problems = await until(
      "the missing template",
      async () => vscode.languages.getDiagnostics(document.uri).map((problem) => problem.message),
      (found) => found.some((message) => message.includes("nope")),
    );
    await vscode.commands.executeCommand("workbench.action.files.revert");
    assert.deepEqual(problems, ["Template 'blog/nope.html.twig' not found"]);
    return problems.join("; ");
  }],
];

exports.run = async function run() {
  const results = [];
  const byMode = {
    "two-projects": twoProjectChecks,
    wordpress: wordpressChecks,
    drupal: drupalChecks,
    codeigniter: codeigniterChecks,
    symfony: symfonyChecks,
  };
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
