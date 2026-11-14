import assert from "node:assert/strict";
import { test } from "node:test";
import { columnSource, embedJson, escapeHtml, modelDetails, modelViewPage, newModelPage } from "../src/laravel/html.ts";
import {
  type ArtisanCommand,
  type GenerationContext,
  type ModelView,
  argumentsHelp,
  artisanCommandLine,
  chosenColumns,
  columnsError,
  emptyClassSpec,
  emptyModelForm,
  generatorEntries,
  generatorTitle,
  migrationTimestamp,
  missingLocales,
  nameError,
  nameKind,
  needsArguments,
  newModelError,
  newModelSpec,
  statusError,
  translationRequestOf,
} from "../src/laravel/model.ts";

const generator = (id: string, title = id) => ({ id, title, description: `${title}.`, templates: [], options: [], fields: [] });

const context = (ids: string[]): GenerationContext => ({
  namespace: "App\\",
  modules: [],
  defaultModule: null,
  models: ["App\\Models\\Post"],
  generators: ids.map((id) => generator(id)),
});

test("the generator list groups what the server offers", () => {
  const entries = generatorEntries(context(["model", "controller", "enum", "ide-json:acme.dto", "brand-new"]));
  assert.deepEqual(
    entries.map((e) => ("heading" in e ? `# ${e.heading}` : e.id)),
    ["model", "# HTTP Layer", "controller", "# Other", "enum", "# ide.json", "ide-json:acme.dto", "# More", "brand-new"],
  );
  // Livewire is only there when the server lists it.
  assert.ok(!generatorEntries(context(["model"])).some((e) => "id" in e && e.id === "livewire"));
  assert.equal(generatorTitle(context(["controller"]), "controller"), "New Controller");
  assert.equal(generatorTitle(context(["ide-json:x"]), "ide-json:x"), "New ide-json:x");
});

test("names are checked by what the generator makes", () => {
  assert.equal(nameKind("pivot-migration"), "none");
  assert.equal(nameError("controller", "Admin/PostController"), null);
  assert.match(nameError("controller", "post controller") ?? "", /not a valid class name/);
  assert.equal(nameError("view", "posts.index"), null);
  assert.equal(nameError("view", "posts/index.blade.php"), null);
  assert.match(nameError("view", "posts index") ?? "", /view name/);
  assert.equal(nameError("migration", "add_votes_to_users_table"), null);
  assert.equal(nameError("livewire", "posts.create-post"), null);
  assert.equal(nameError("pivot-migration", ""), null);
  assert.equal(nameError("job", "  "), "Enter a name");
  assert.equal(statusError(""), null);
  assert.equal(statusError("403"), null);
  assert.match(statusError("40") ?? "", /three-digit/);
  assert.equal(emptyClassSpec("job").queued, true);
});

test("the New Model form becomes the server's request", () => {
  const form = emptyModelForm("Blog");
  form.name = " Post ";
  form.fields = [
    { name: " title ", ty: "string", nullable: false, unique: true, index: false, default: "" },
    { name: "", ty: "text", nullable: false, unique: false, index: false, default: "" },
  ];
  form.relations = [
    { kind: "belongsTo", model: "App\\Models\\User", name: "author" },
    { kind: "hasMany", model: "", name: "" },
    { kind: "morphTo", model: "ignored", name: "commentable" },
  ];
  form.indexes = [
    { columns: "status, published_at", unique: false },
    { columns: " ", unique: true },
  ];
  const spec = newModelSpec(form);
  assert.equal(spec.name, "Post");
  assert.equal(spec.module, "Blog");
  assert.deepEqual(spec.fields.map((f) => f.name), ["title"]);
  assert.deepEqual(spec.relations.map((r) => [r.kind, r.model, r.name]), [
    ["belongsTo", "App\\Models\\User", "author"],
    ["morphTo", "", "commentable"],
  ]);
  assert.deepEqual(spec.indexes, [{ columns: ["status", "published_at"], unique: false }]);
  assert.equal(newModelError(form), null);
});

test("the New Model form says what is wrong", () => {
  const form = emptyModelForm();
  assert.equal(newModelError(form), "Enter the model's class name");
  form.name = "Post";
  assert.equal(newModelError(form), null);
  const field = (name: string) => ({ name, ty: "string", nullable: false, unique: false, index: false, default: "" });
  assert.match(newModelError({ ...form, fields: [field("my title")] }) ?? "", /not a valid column name/);
  assert.match(newModelError({ ...form, fields: [field("title"), field("Title")] }) ?? "", /twice/);
  assert.match(newModelError({ ...form, relations: [{ kind: "morphTo", model: "", name: "" }] }) ?? "", /morphTo/);
  assert.match(newModelError({ ...form, indexes: [{ columns: "a b", unique: false }] }) ?? "", /column name/);
  // A controller kind the page can't send falls back to none.
  assert.equal(newModelSpec({ ...form, controller: "odd" }).controller, "none");
});

test("columns to add are checked against the table", () => {
  const rows = [
    { name: " subtitle ", ty: "string", nullable: true },
    { name: "", ty: "text", nullable: false },
  ];
  assert.deepEqual(chosenColumns(rows), [{ name: "subtitle", ty: "string", nullable: true }]);
  assert.equal(columnsError(rows, ["id", "title"]), null);
  assert.match(columnsError(rows, ["Subtitle"]) ?? "", /already/);
  assert.equal(columnsError([{ name: " ", ty: "string", nullable: false }], []), "Enter at least one column name");
  assert.match(columnsError([{ name: "1st", ty: "string", nullable: false }], []) ?? "", /not a valid column name/);
});

test("migration timestamps are local and zero-padded", () => {
  assert.equal(migrationTimestamp(new Date(2024, 4, 1, 9, 5, 7)), "2024_05_01_090507");
});

const command = (over: Partial<ArtisanCommand> = {}): ArtisanCommand => ({
  name: "posts:prune-drafts",
  description: "Prune old drafts",
  synopsis: "posts:prune-drafts [--force] [<days>]",
  arguments: [{ name: "days", description: "Age in days", required: false, array: false, default: "30" }],
  options: [{ name: "force", shortcut: null, description: "", acceptsValue: false, array: false, default: null }],
  source: "class",
  class: "App\\Console\\Commands\\PruneDrafts",
  location: null,
  ...over,
});

test("Artisan command lines and their help", () => {
  assert.equal(artisanCommandLine("php artisan", "migrate", " --seed "), "php artisan migrate --seed");
  assert.equal(artisanCommandLine(" ", "migrate"), "php artisan migrate");
  assert.equal(artisanCommandLine("./vendor/bin/sail artisan", "queue:work"), "./vendor/bin/sail artisan queue:work");
  assert.equal(needsArguments(command()), false);
  assert.equal(needsArguments(command({ arguments: [{ ...command().arguments[0]!, required: true }] })), true);
  assert.equal(argumentsHelp(command()), "[days]: Age in days · --force");
  assert.equal(argumentsHelp(command({ arguments: [], options: [] })), "");
});

test("the translation request of the action's argument", () => {
  const request = translationRequestOf(
    JSON.stringify({
      key: "messages.hi",
      locales: [
        { locale: "en", text: "Hi", exists: true },
        { locale: "fr", text: "Hi" },
        { nope: true },
      ],
    }),
  );
  assert.deepEqual(request?.locales, [
    { locale: "en", text: "Hi", exists: true },
    { locale: "fr", text: "Hi", exists: false },
  ]);
  assert.deepEqual(missingLocales(request!).map((l) => l.locale), ["fr"]);
  assert.equal(translationRequestOf("not json"), null);
  assert.equal(translationRequestOf({ key: 1 }), null);
});

const resources = { cspSource: "vscode-webview://abc", style: "vscode-webview://abc/laravel.css", script: "vscode-webview://abc/x.js" };

const place = (path: string) => ({ path, range: { start: { line: 4, col: 6 }, end: { line: 4, col: 10 } } });

const view: ModelView = {
  class: "App\\Models\\Post",
  name: "Post",
  location: place("/app/Models/Post.php"),
  table: "posts",
  primaryKey: "id",
  keyType: "int",
  incrementing: true,
  schemaCertain: false,
  columns: [
    {
      name: "title",
      ty: "string",
      nullable: true,
      cast: "\\App\\Casts\\Title<x>",
      source: "migration",
      database: null,
      fillable: true,
      hidden: false,
      location: place("/db/1_create_posts.php"),
    },
  ],
  relations: [{ name: "author", kind: "BelongsTo", related: "App\\Models\\User", many: false, location: null }],
  scopes: [],
  accessors: [{ name: "excerpt", method: "excerpt", ty: "string", location: place("/app/Models/Post.php") }],
  related: [{ kind: "migration", name: "1_create_posts", location: place("/db/1_create_posts.php") }],
};

test("the model view's details", () => {
  const html = modelDetails(view);
  assert.match(html, /<code>\?string<\/code>/);
  // A cast is shown without its leading backslash, and escaped.
  assert.ok(html.includes("<code>App\\Casts\\Title&lt;x&gt;</code>"), html);
  assert.ok(html.includes("BelongsTo → User"));
  assert.ok(html.includes("The columns may be incomplete"));
  assert.match(html, /Scopes <span class="count">0<\/span><\/h2>\s*<p class="hint">None<\/p>/);
  // A column goes to its migration but isn't renamed from here; a migration neither.
  const contexts = [...html.matchAll(/data-vscode-context="([^"]*)"/g)].map((m) =>
    JSON.parse(m[1]!.replaceAll("&quot;", '"').replaceAll("&amp;", "&")),
  );
  assert.deepEqual(
    contexts.map((c) => [c.place.path, c.renamable]),
    [
      ["/app/Models/Post.php", true],
      ["/db/1_create_posts.php", false],
      ["/app/Models/Post.php", true],
      ["/db/1_create_posts.php", false],
    ],
  );
  // The relation has no place: plain text.
  assert.ok(html.includes('<span class="name">author</span>'));
  assert.equal(columnSource({ ...view.columns[0]!, source: "database", database: "local" }), "database local");
});

test("pages load only their own files", () => {
  const html = modelViewPage(view, resources);
  assert.ok(html.includes(`content="default-src &#39;none&#39;; style-src vscode-webview://abc; script-src vscode-webview://abc;"`));
  assert.ok(!/<script(?![^>]*(src=|type="application\/json"))/.test(html), "an inline script");
  assert.ok(!/ style="/.test(html), "an inline style");
  assert.ok(html.includes('<form id="add-columns"'));
});

test("page data can't close its script element", () => {
  assert.equal(embedJson({ a: "</script><b>" }), '{"a":"\\u003c/script>\\u003cb>"}');
  assert.equal(escapeHtml(`<a href="x">'&`), "&lt;a href=&quot;x&quot;&gt;&#39;&amp;");
  const ctx = { ...context(["model"]), models: ["App\\Models\\</script>"] };
  const html = newModelPage(ctx, emptyModelForm(), resources);
  assert.equal(html.split("</script>").length - 1, 2, html);
  const data = JSON.parse(/<script type="application\/json" id="data">(.*?)<\/script>/s.exec(html)![1]!);
  assert.deepEqual(data.models, ["App\\Models\\</script>"]);
  assert.equal(data.form.fields.length, 1);
});
