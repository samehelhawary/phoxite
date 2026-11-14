// Drives a php-engine-lsp binary over stdio on a project, as an editor would,
// and prints what a few requests answer and how long they take.
//
//   node scripts/smoke_stdio.mjs <server> <project> <file relative to the project> [line col]
//
// Uses vscode-jsonrpc from editors/vscode/node_modules (run `npm install` there first).

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(path.join(here, "../editors/vscode/package.json"));
const rpc = require("vscode-jsonrpc/node");

const [server, project, rel, line = "0", col = "0"] = process.argv.slice(2);
if (!server || !project || !rel) {
  console.error("usage: node scripts/smoke_stdio.mjs <server> <project> <file> [line col]");
  process.exit(2);
}
const file = path.resolve(project, rel);
const uri = pathToFileURL(file).href;
const child = spawn(server, ["--stdio"], { stdio: ["pipe", "pipe", "inherit"] });
const conn = rpc.createMessageConnection(
  new rpc.StreamMessageReader(child.stdout),
  new rpc.StreamMessageWriter(child.stdin),
);
let diagnostics = [];
let indexing = "";
conn.onNotification("textDocument/publishDiagnostics", (p) => {
  if (p.uri === uri) diagnostics = p.diagnostics;
});
conn.onNotification("$/progress", (p) => (indexing = p.value.kind));
conn.onRequest("window/workDoneProgress/create", () => null);
conn.listen();

const timed = async (label, method, params) => {
  const started = performance.now();
  const result = await conn.sendRequest(method, params);
  const ms = (performance.now() - started).toFixed(0);
  const summary = Array.isArray(result) ? `${result.length} results` : result === null ? "null" : JSON.stringify(result).slice(0, 160);
  console.log(`${label.padEnd(20)} ${ms.padStart(6)} ms  ${summary}`);
  return result;
};

const started = performance.now();
const init = await conn.sendRequest("initialize", {
  processId: process.pid,
  rootUri: null,
  workspaceFolders: [{ uri: pathToFileURL(path.resolve(project)).href, name: "project" }],
  capabilities: {
    window: { workDoneProgress: true },
    workspace: { workspaceEdit: { documentChanges: true, resourceOperations: ["create", "rename", "delete"] } },
    textDocument: { completion: { completionItem: { snippetSupport: true, insertReplaceSupport: true } } },
  },
  initializationOptions: { settings: {} },
});
conn.sendNotification("initialized", {});
console.log(`server ${init.serverInfo.name} ${init.serverInfo.version}`);
conn.sendNotification("textDocument/didOpen", {
  textDocument: { uri, languageId: rel.endsWith(".blade.php") ? "blade" : "php", version: 1, text: readFileSync(file, "utf8") },
});

// Wait for indexing to end (the progress `end`), at most two minutes.
const deadline = Date.now() + 120_000;
while (indexing !== "end" && Date.now() < deadline) await new Promise((r) => setTimeout(r, 200));
console.log(`indexed in ${((performance.now() - started) / 1000).toFixed(1)} s`);

const doc = { textDocument: { uri } };
const at = { ...doc, position: { line: Number(line), character: Number(col) } };
await timed("documentSymbol", "textDocument/documentSymbol", doc);
await timed("hover", "textDocument/hover", at);
await timed("definition", "textDocument/definition", at);
await timed("references", "textDocument/references", { ...at, context: { includeDeclaration: true } });
const completion = await timed("completion", "textDocument/completion", at);
if (completion?.items) console.log(`  first items: ${completion.items.slice(0, 8).map((i) => i.label).join(", ")}`);
await timed("semanticTokens", "textDocument/semanticTokens/full", doc);
await timed("inlayHint", "textDocument/inlayHint", { ...doc, range: { start: { line: 0, character: 0 }, end: { line: 9999, character: 0 } } });
await timed("codeLens", "textDocument/codeLens", doc);
await timed("formatting", "textDocument/formatting", { ...doc, options: { tabSize: 4, insertSpaces: true } });
await timed("workspace/symbol", "workspace/symbol", { query: path.basename(rel, ".php") });
await new Promise((r) => setTimeout(r, 1500));
console.log(`diagnostics          ${diagnostics.length} for the file`);
for (const d of diagnostics.slice(0, 5)) console.log(`  ${d.range.start.line + 1}: [${d.code}] ${d.message}`);

await conn.sendRequest("shutdown");
conn.sendNotification("exit");
setTimeout(() => process.exit(0), 500);
