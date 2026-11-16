// The VS Code client of phoxite-lsp.

import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import * as vscode from "vscode";
import {
  type Executable,
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  type ServerOptions,
  State,
} from "vscode-languageclient/node";
import { guarded, message, running } from "./errors";
import { Laravel } from "./laravel/api";
import { registerCommands } from "./laravel/commands";
import { registerPages } from "./laravel/pages";
import { registerRoutes } from "./laravel/routesView";
import { Refactorings, registerRefactorings } from "./refactor/commands";
import { openable, parseRoutes, routePicks } from "./routes";
import { bundledName, configuredServer, serverProblem } from "./serverPath";

let client: LanguageClient | undefined;
let output: vscode.LogOutputChannel | undefined;
/** What listens to the running client; gone with it. */
const listening: vscode.Disposable[] = [];
/** The restart under way: a second request waits for it instead of starting another server. */
let restarting: Promise<void> | undefined;
/** The Laravel requests of the server, and whether the workspace is a Laravel project. */
const laravel = new Laravel(() => client);
/** The refactorings that ask something first. */
const refactorings = new Refactorings(() => client);

/** Other PHP language servers that would answer the same requests. */
const OTHER_SERVERS = ["bmewburn.vscode-intelephense-client", "zobo.php-intellisense", "phpactor.vscode-phpactor"];

/** What other extensions (and the end-to-end tests) get from this one. */
export interface PhoxiteApi {
  laravel: Laravel;
  refactorings: Refactorings;
}

export async function activate(context: vscode.ExtensionContext): Promise<PhoxiteApi> {
  output = vscode.window.createOutputChannel("Phoxite", { log: true });
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("phoxite.restart", () => restart(context)),
    vscode.commands.registerCommand("phoxite.showOutput", () => output?.show()),
    vscode.commands.registerCommand(
      "phoxite.rescan",
      guarded(async () => {
        await running(client).sendRequest("workspace/executeCommand", { command: "phoxite.server.rescan", arguments: [] });
        vscode.window.setStatusBarMessage("Phoxite: indexing the project again", 3000);
      }),
    ),
    vscode.commands.registerCommand("phoxite.openLocation", openLocation),
    vscode.commands.registerCommand("laravel.routeActions", routeActions),
    // A project becomes a Laravel one (or stops being one) with its Composer files.
    vscode.workspace.onDidChangeWorkspaceFolders(() => void laravel.refreshStatus()),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (/[\\/]composer\.(json|lock)$/.test(doc.fileName)) void laravel.refreshStatus();
    }),
    vscode.workspace.onDidGrantWorkspaceTrust(() => {
      if (client?.isRunning()) void client.sendNotification("phoxite/workspaceTrust", { trusted: true });
    }),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("phoxite.laravel.detection")) void laravel.refreshStatus();
      // Another executable is used from the next start on.
      if (e.affectsConfiguration("phoxite.server.path")) {
        void vscode.window
          .showInformationMessage("Phoxite: restart the language server to use the new `phoxite.server.path`.", "Restart")
          .then((choice) => choice && restart(context));
      }
    }),
    laravel,
  );
  registerRoutes(context, laravel);
  registerPages(context, laravel);
  registerCommands(context, laravel);
  registerRefactorings(context, refactorings);
  warnAboutOtherServers();
  await start(context);
  return { laravel, refactorings };
}

export async function deactivate(): Promise<void> {
  await stop();
}

/** Stops the server, if one runs. A client that never started has nothing to stop. */
async function stop(): Promise<void> {
  const stopping = client;
  client = undefined;
  for (const listener of listening.splice(0)) listener.dispose();
  await laravel.reset();
  if (!stopping) return;
  try {
    // `stop` throws on a client whose start failed, and that is when Restart is needed.
    if (stopping.needsStop()) await stopping.stop();
  } catch (e) {
    output?.warn(`stopping the language server: ${message(e)}`);
  }
}

/** The server executable (the setting, else the one bundled for this platform), or why there is none. */
function server(context: vscode.ExtensionContext): { command: string; problem: string | null } {
  const configured = configuredServer(vscode.workspace.getConfiguration("phoxite").get<string>("server.path"), {
    home: os.homedir(),
    folder: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
  });
  const bundled = path.join(context.extensionPath, "server", bundledName(process.platform));
  return { command: configured ?? bundled, problem: serverProblem(configured, bundled, fs.existsSync) };
}

/** Says the server isn't there, with what helps; doesn't wait for the answer. */
function notStarted(context: vscode.ExtensionContext, why: string): void {
  output?.error(why);
  void vscode.window.showErrorMessage(`Phoxite: ${why}`, "Show Log", "Open Settings", "Restart").then((choice) => {
    if (choice === "Show Log") output?.show();
    else if (choice === "Open Settings") void vscode.commands.executeCommand("workbench.action.openSettings", "phoxite.server.path");
    else if (choice === "Restart") void restart(context);
  });
}

function settings(): unknown {
  return vscode.workspace.getConfiguration("phoxite");
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const { command, problem } = server(context);
  if (problem) {
    notStarted(context, problem);
    return;
  }
  const run: Executable = { command, args: ["--stdio"], options: { env: { ...process.env } } };
  const serverOptions: ServerOptions = { run, debug: { ...run, options: { env: { ...process.env, PHOXITE_LOG: "debug" } } } };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "php" },
      { scheme: "file", language: "blade" },
      // Environment keys and JSON translations: usages and rename.
      { scheme: "file", pattern: "**/.env*" },
      { scheme: "file", pattern: "**/lang/**/*.json" },
    ],
    // The server registers the files it wants watched itself
    // (`workspace/didChangeWatchedFiles`), as for every other editor.
    synchronize: { configurationSection: "phoxite" },
    initializationOptions: {
      cacheDir: path.join(context.globalStorageUri.fsPath, "index"),
      settings: settings(),
      // PHPStan and Psalm run code of the project: only in a trusted workspace.
      trusted: vscode.workspace.isTrusted,
    },
    outputChannel: output,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
  };
  const starting = new LanguageClient("phoxite", "Phoxite", serverOptions, clientOptions);
  client = starting;
  let attached = false;
  listening.push(
    starting.onDidChangeState((e) => {
      // The server stopped (or crashed): nothing is known about the project until it
      // runs again. The client restarts a crashed server itself; then it is asked anew.
      if (e.newState === State.Stopped) void laravel.reset();
      else if (e.newState === State.Running && attached) void laravel.refreshStatus();
    }),
  );
  // This client has the dialogs of the refactorings that ask something first, so
  // the server offers them (`refactor/commands.ts`).
  starting.registerFeature({
    fillClientCapabilities(capabilities) {
      const experimental = (capabilities.experimental ?? {}) as Record<string, unknown>;
      capabilities.experimental = { ...experimental, phoxite: { dialogs: true } };
    },
    initialize() {},
    getState: () => ({ kind: "static" }),
    clear() {},
  });
  try {
    await starting.start();
  } catch (e) {
    notStarted(context, `the language server (${command}) didn't start: ${message(e)}`);
    return;
  }
  // Another start or a stop came meanwhile: this client is not the one in use.
  if (client !== starting) return;
  attached = true;
  listening.push(laravel.attach(starting));
}

/** Stops the server and starts it again; requests made while one is under way share it. */
function restart(context: vscode.ExtensionContext): Promise<void> {
  restarting ??= (async () => {
    try {
      await stop();
      await start(context);
    } finally {
      restarting = undefined;
    }
  })();
  return restarting;
}

function warnAboutOtherServers(): void {
  if (!vscode.workspace.getConfiguration("phoxite").get<boolean>("warnAboutOtherServers", true)) return;
  const others = OTHER_SERVERS.map((id) => vscode.extensions.getExtension(id)).filter((e) => e !== undefined);
  if (others.length === 0) return;
  const names = others.map((e) => e.packageJSON.displayName ?? e.id).join(", ");
  void vscode.window
    .showWarningMessage(
      `Phoxite: ${names} is enabled too, so completions and problems show twice. Disable it for this workspace?`,
      "Show Extension",
      "Don't Ask Again",
    )
    .then((choice) => {
      if (choice === "Show Extension") {
        void vscode.commands.executeCommand("workbench.extensions.search", `@id:${others[0].id}`);
      } else if (choice === "Don't Ask Again") {
        void vscode.workspace.getConfiguration("phoxite").update("warnAboutOtherServers", false, true);
      }
    });
}

/** Opens a location the server names (after a quick-fix that creates a file). */
async function openLocation(location: { uri: string; range: vscode.Range }): Promise<void> {
  if (!location?.uri) return;
  const uri = vscode.Uri.parse(location.uri);
  const r = location.range;
  const selection = new vscode.Range(r.start.line, r.start.character, r.end.line, r.end.character);
  await vscode.window.showTextDocument(uri, { selection });
}

/** The actions of the routes of a route lens or hint. */
async function routeActions(arg: unknown): Promise<void> {
  const routes = parseRoutes(arg);
  if (routes.length === 0) return;
  const picked = await vscode.window.showQuickPick(routePicks(routes), { placeHolder: "Route" });
  if (!picked) return;
  const { route } = picked;
  switch (picked.action) {
    case "goto": {
      const s = route.range.start;
      const e = route.range.end;
      await vscode.window.showTextDocument(vscode.Uri.file(route.path), {
        selection: new vscode.Range(s.line, s.col, e.line, e.col),
      });
      break;
    }
    case "copy":
      await vscode.env.clipboard.writeText(route.url);
      void vscode.window.setStatusBarMessage(`Copied ${route.url}`, 3000);
      break;
    case "open":
      if (openable(route.url)) {
        await vscode.env.openExternal(vscode.Uri.parse(route.url));
      } else {
        const url = await vscode.window.showInputBox({ prompt: "Fill in the route's parameters", value: route.url });
        if (url) await vscode.env.openExternal(vscode.Uri.parse(url));
      }
      break;
    case "request":
      await vscode.env.clipboard.writeText(route.request);
      void vscode.window.setStatusBarMessage("Copied the HTTP request", 3000);
      break;
  }
}
