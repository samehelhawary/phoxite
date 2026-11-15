// Captures what a real phoxite-lsp answers on the Laravel fixture, as the files under
// tests/fixtures/ that the plugin's tests parse. Run it again when the server's
// answers change shape:
//
//   node editors/openidea/tests/capture.mjs target/release/phoxite-lsp
//
// Uses vscode-jsonrpc from editors/vscode/node_modules (run `npm install` there first).
// The project's path is written as /project, so the files don't depend on the machine.

import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
const require = createRequire(path.join(repo, "editors/vscode/package.json"));
const rpc = require("vscode-jsonrpc/node");

const server = process.argv[2];
if (!server) {
  console.error("usage: node editors/openidea/tests/capture.mjs <phoxite-lsp>");
  process.exit(2);
}
const project = path.join(repo, "fixtures/laravel-app");
const out = path.join(here, "fixtures");
mkdirSync(out, { recursive: true });

const child = spawn(server, ["--stdio"], { stdio: ["pipe", "pipe", "inherit"] });
const conn = rpc.createMessageConnection(
  new rpc.StreamMessageReader(child.stdout),
  new rpc.StreamMessageWriter(child.stdin),
);
conn.onRequest("window/workDoneProgress/create", () => null);
conn.onRequest("workspace/configuration", () => []);
conn.onRequest("client/registerCapability", () => null);
conn.listen();

// What OpenIDEA's client declares for edits: document changes with file creation.
await conn.sendRequest("initialize", {
  processId: process.pid,
  rootUri: null,
  workspaceFolders: [{ uri: pathToFileURL(project).href, name: "project" }],
  capabilities: {
    window: { workDoneProgress: true },
    workspace: { workspaceEdit: { documentChanges: true, resourceOperations: ["create", "rename", "delete"] } },
  },
  initializationOptions: { settings: {} },
});
conn.sendNotification("initialized", {});

const laravel = (name, params = {}) => conn.sendRequest(`phoxite/laravel/${name}`, params);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// The index is built in the background: wait until the routes are there.
let routes = [];
for (let attempt = 0; attempt < 120 && routes.length === 0; attempt++) {
  routes = await laravel("routes", { query: "" }).catch(() => []);
  if (routes.length === 0) await sleep(500);
}
if (routes.length === 0) throw new Error("the server answered no routes for the fixture");

// The project's path, in each of the forms an answer can carry it.
const forward = project.replaceAll("\\", "/");
const uriPath = pathToFileURL(project).href.slice("file://".length);
const normalize = (value) =>
  JSON.parse(
    JSON.stringify(value)
      .replaceAll(JSON.stringify(project).slice(1, -1), "/project")
      .replaceAll(uriPath, "/project")
      .replaceAll(forward, "/project")
      .replaceAll(/\/project((?:\\\\|\/)[^"]*)/g, (_, rest) => "/project" + rest.replaceAll("\\\\", "/")),
  );
const save = (name, value) => {
  writeFileSync(path.join(out, `${name}.json`), JSON.stringify(normalize(value), null, 2) + "\n");
  console.log(`${name}: ${Array.isArray(value) ? `${value.length} items` : value === null ? "null" : "ok"}`);
};

const context = await laravel("generationContext");
const model = context.models.find((m) => m.endsWith("\\Post")) ?? context.models[0];
const spec = (generator, name, extra = {}) => ({
  generator, name, module: null, model: "", related: "", table: "", create: false, event: "",
  queued: true, api: false, backing: "", status: 0, inputs: {}, ...extra,
});

save("routes", routes.slice(0, 6));
save("modelView", await laravel("modelView", { class: model }));
// The project's own commands and a few of the framework's: the full list is long.
const commands = await laravel("artisanCommands");
const own = commands.filter((c) => c.source !== "framework" && c.source !== "package");
save("artisanCommands", [...own, ...commands.filter((c) => !own.includes(c)).slice(0, 3)]);
save("generationContext", context);
save("generateClass", await laravel("generateClass", { spec: spec("controller", "CapturedController", { model }), timestamp: "2024_05_01_120000" }));
save("addColumns", await laravel("addColumns", {
  class: model,
  columns: [{ name: "captured_votes", ty: "integer", nullable: true }],
  fillable: true,
  timestamp: "2024_05_01_120000",
}));
save("newModel", await laravel("newModel", {
  spec: {
    name: "CapturedThing", module: "",
    fields: [{ name: "title", ty: "string", nullable: false, unique: false, index: false, default: "" }],
    relations: [], indexes: [], timestamps: true, softDeletes: false,
    migration: true, factory: true, seeder: false, controller: "resource",
    formRequests: false, resource: false, policy: false,
  },
  timestamp: "2024_05_01_120000",
}));
save("createTranslation", await laravel("createTranslation", {
  key: "captured.greeting",
  texts: [{ locale: "fr", text: "Bonjour" }],
}));

await conn.sendRequest("shutdown");
conn.sendNotification("exit");
setTimeout(() => process.exit(0), 200);
