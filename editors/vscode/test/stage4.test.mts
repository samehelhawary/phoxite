import assert from "node:assert/strict";
import { test } from "node:test";
import { messagePage, modelErrorPage, newModelPage } from "../src/laravel/html.ts";
import { chosenFolder, emptyModelForm } from "../src/laravel/model.ts";
import { changeSignaturePage } from "../src/refactor/html.ts";
import { type Route, routeMatches, routesState } from "../src/routes.ts";
import { type Outcome, runSteps } from "../src/steps.ts";

const resources = { cspSource: "vscode-resource:", style: "laravel.css", script: "page.js" };

test("a request is about the active editor's project when it is a Laravel one", () => {
  const app = "file:///w/app";
  const shop = "file:///w/shop";
  assert.equal(chosenFolder(shop, [app, shop]), shop);
  // An editor of a folder that is no Laravel project, or no editor: the first project.
  assert.equal(chosenFolder("file:///w/docs", [app, shop]), app);
  assert.equal(chosenFolder(undefined, [app, shop]), app);
  // A server that doesn't say where the projects are picks itself, from the editor's folder.
  assert.equal(chosenFolder(shop, []), shop);
  assert.equal(chosenFolder(undefined, []), undefined);
});

const route = (over: Partial<Route>): Route => ({
  name: "posts.show",
  methods: ["GET", "HEAD"],
  uri: "posts/{post}",
  action: "App\\Http\\Controllers\\PostController@show",
  middleware: [],
  path: "/w/app/routes/web.php",
  range: { start: { line: 0, col: 0 }, end: { line: 0, col: 1 } },
  url: "http://localhost/posts/1",
  request: "GET http://localhost/posts/1",
  ...over,
});

test("the routes filter matches every word, in the method, URI, name or action", () => {
  const show = route({});
  assert.ok(routeMatches(show, ""));
  assert.ok(routeMatches(show, "  "));
  assert.ok(routeMatches(show, "posts"));
  assert.ok(routeMatches(show, "GET /posts"));
  assert.ok(routeMatches(show, "postcontroller show"));
  assert.ok(routeMatches(show, "posts.show"));
  assert.ok(!routeMatches(show, "posts delete"));
  assert.ok(!routeMatches(route({ name: null }), "posts.show"));
});

test("an empty routes view says why it is empty", () => {
  assert.equal(routesState(undefined, 0), "failed");
  assert.equal(routesState(0, 0), "empty");
  assert.equal(routesState(12, 0), "filtered");
  assert.equal(routesState(12, 3), "listed");
});

test("steps are asked in order, Back asks the one before again, giving up ends them", async () => {
  const asked: string[] = [];
  const step = (name: string, outcomes: Outcome[]) => async (at: { step: number; total: number }) => {
    asked.push(`${name} ${at.step}/${at.total}`);
    return outcomes.shift() ?? "next";
  };
  assert.equal(await runSteps([step("name", []), step("module", ["back"]), step("options", [])]), true);
  assert.deepEqual(asked, ["name 1/3", "module 2/3", "name 1/3", "module 2/3", "options 3/3"]);

  asked.length = 0;
  assert.equal(await runSteps([step("name", ["back"]), step("module", ["cancel"]), step("options", [])]), false);
  // Back at the first step stays there; nothing is asked after giving up.
  assert.deepEqual(asked, ["name 1/3", "name 1/3", "module 2/3"]);
  assert.equal(await runSteps([]), true);
});

test("a panel's page carries what brings it back after a window reload", () => {
  const ctx = { namespace: "App", modules: [], models: [], defaultModule: null, generators: [] };
  const form = newModelPage(ctx as never, emptyModelForm(), resources, { folder: "file:///w/shop" });
  assert.match(form, /"panel":\{"folder":"file:\/\/\/w\/shop"\}/);
  const failed = modelErrorPage("App\\Models\\Post", "The language server is not running", resources, {
    class: "App\\Models\\Post",
    folder: "file:///w/shop",
  });
  assert.match(failed, /"panel":\{"class":"App\\\\Models\\\\Post","folder":"file:\/\/\/w\/shop"\}/);
  const signature = {
    name: "Shop::total",
    functionName: "total",
    visibility: "public",
    returnType: "int",
    constructor: false,
    parameters: [],
  };
  const page = changeSignaturePage(signature as never, resources, { uri: "file:///w/app/Shop.php", data: { signature } });
  assert.match(page, /"panel":\{"uri":"file:\/\/\/w\/app\/Shop\.php"/);
});

test("a page that only says something runs no script", () => {
  const page = messagePage("New Eloquent Model", "Waiting for <the server>…", resources);
  assert.doesNotMatch(page, /<script/);
  assert.match(page, /Waiting for &lt;the server&gt;…/);
});
