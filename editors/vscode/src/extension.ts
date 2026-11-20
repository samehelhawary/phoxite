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
import { type Other, OTHERS, conflictMessage, seenKey } from "./conflicts";
import { guarded, message, registerServerCommand, running } from "./errors";
import { Laravel } from "./laravel/api";
import { registerCommands } from "./laravel/commands";
import { registerPages } from "./laravel/pages";
import { registerRoutes } from "./laravel/routesView";
import { Refactorings, registerRefactorings } from "./refactor/commands";
import { openable, parseRoutes, routePicks } from "./routes";
import { withOwnCommand } from "./serverCommands";
import { bundledName, configuredServer, serverProblem } from "./serverPath";
import { type ServerStatus, statusView } from "./status";

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

/** The token of the server's indexing progress. */
const INDEXING = "phoxite/indexing";

/** What is known about the server, and the item (on PHP and Blade files) that shows it. */
const status: ServerStatus = { server: "starting", indexing: false, laravel: false };
let statusItem: vscode.LanguageStatusItem | undefined;

function show(change: Partial<ServerStatus>): void {
  Object.assign(status, change);
  if (!statusItem) return;
  const view = statusView(status);
  statusItem.text = view.text;
  statusItem.detail = view.detail;
  statusItem.busy = view.busy;
  statusItem.severity =
    view.severity === "error" ? vscode.LanguageStatusSeverity.Error : vscode.LanguageStatusSeverity.Information;
}

/** What the status item offers. */
async function statusActions(context: vscode.ExtensionContext): Promise<void> {
  const view = statusView(status);
  const actions: Array<vscode.QuickPickItem & { run: () => unknown }> = [
    { label: "$(debug-restart) Restart the Language Server", run: () => restart(context) },
    { label: "$(output) Show the Server Log", run: () => output?.show() },
    { label: "$(sync) Index the Project Again", run: () => vscode.commands.executeCommand("phoxite.rescan") },
    {
      label: "$(gear) Open the Settings",
      run: () => vscode.commands.executeCommand("workbench.action.openSettings", "@ext:phoxite.phoxite"),
    },
  ];
  const picked = await vscode.window.showQuickPick(actions, { title: view.text, placeHolder: view.detail });
  await picked?.run();
}

/** What other extensions (and the end-to-end tests) get from this one. */
export interface PhoxiteApi {
  laravel: Laravel;
  refactorings: Refactorings;
}

export async function activate(context: vscode.ExtensionContext): Promise<PhoxiteApi> {
  output = vscode.window.createOutputChannel("Phoxite", { log: true });
  statusItem = vscode.languages.createLanguageStatusItem("phoxite.status", [{ language: "php" }, { language: "blade" }, { language: "twig" }]);
  statusItem.name = "Phoxite";
  statusItem.command = { title: "Actions", command: "phoxite.status" };
  show({});
  context.subscriptions.push(
    output,
    statusItem,
    laravel.onDidChange((active) => show({ laravel: active })),
    vscode.commands.registerCommand(
      "phoxite.status",
      guarded(() => statusActions(context)),
    ),
    vscode.commands.registerCommand("phoxite.restart", () => restart(context)),
    vscode.commands.registerCommand("phoxite.showOutput", () => output?.show()),
    vscode.commands.registerCommand("phoxite.showLog", () => output?.show()),
    vscode.commands.registerCommand(
      "phoxite.rescan",
      guarded(async () => {
        await running(client).sendRequest("workspace/executeCommand", { command: "phoxite.server.rescan", arguments: [] });
        vscode.window.setStatusBarMessage("Phoxite: indexing the project again", 3000);
      }),
    ),
    vscode.commands.registerCommand("phoxite.openLocation", openLocation),
    registerServerCommand("laravel.routeActions", routeActions),
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
      // Another executable, or another log level, is used from the next start on.
      const setting = ["phoxite.server.path", "phoxite.server.logLevel"].find((s) => e.affectsConfiguration(s));
      if (setting) {
        void vscode.window
          .showInformationMessage(`Phoxite: restart the language server to use the new \`${setting}\`.`, "Restart")
          .then((choice) => choice && restart(context));
      }
    }),
    laravel,
  );
  registerRoutes(context, laravel);
  registerPages(context, laravel);
  registerCommands(context, laravel);
  registerRefactorings(context, refactorings);
  warnAboutOtherExtensions(context);
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
  show({ server: "failed", problem: why, indexing: false });
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
  show({ server: "starting", problem: undefined, indexing: false, indexed: undefined });
  const { command, problem } = server(context);
  if (problem) {
    notStarted(context, problem);
    return;
  }
  const level = vscode.workspace.getConfiguration("phoxite").get<string>("server.logLevel", "info");
  const run: Executable = { command, args: ["--stdio"], options: { env: { ...process.env, PHOXITE_LOG: level } } };
  const serverOptions: ServerOptions = { run, debug: { ...run, options: { env: { ...process.env, PHOXITE_LOG: "debug" } } } };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "php" },
      { scheme: "file", language: "blade" },
      { scheme: "file", language: "twig" },
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
    middleware: {
      // The status item follows the server's indexing.
      handleWorkDoneProgress(token, params, next) {
        if (token === INDEXING) {
          if (params.kind === "begin") show({ indexing: true });
          else if (params.kind === "end") show({ indexing: false, indexed: params.message });
        }
        next(token, params);
      },
      // The commands the server's lenses, hints and actions carry run under this
      // extension's own names (`serverCommands.ts`).
      async provideCodeLenses(document, token, next) {
        return (await next(document, token))?.map(withOwnCommand);
      },
      async resolveCodeLens(lens, token, next) {
        const resolved = await next(lens, token);
        return resolved ? withOwnCommand(resolved) : resolved;
      },
      async provideCodeActions(document, range, context, token, next) {
        const actions = await next(document, range, context, token);
        // A bare command names itself in `command` as a string: only its own field is one.
        return actions?.map((action) =>
          action instanceof vscode.CodeAction ? withOwnCommand(action) : { ...action, command: rewrittenCommand(action.command) },
        );
      },
      async provideInlayHints(document, range, token, next) {
        const hints = await next(document, range, token);
        for (const hint of hints ?? []) {
          if (Array.isArray(hint.label)) hint.label.forEach(withOwnCommand);
        }
        return hints;
      },
      async resolveInlayHint(hint, token, next) {
        const resolved = await next(hint, token);
        if (resolved && Array.isArray(resolved.label)) resolved.label.forEach(withOwnCommand);
        return resolved;
      },
    },
  };
  const starting = new LanguageClient("phoxite", "Phoxite", serverOptions, clientOptions);
  client = starting;
  let attached = false;
  listening.push(
    starting.onDidChangeState((e) => {
      // The server stopped (or crashed): nothing is known about the project until it
      // runs again. The client restarts a crashed server itself; then it is asked anew.
      if (e.newState === State.Stopped) {
        void laravel.reset();
        // A start that failed says why; this is a server that ran and went away.
        if (status.server !== "failed") show({ server: "stopped", indexing: false });
      } else if (e.newState === State.Starting) {
        show({ server: "starting", problem: undefined });
      } else if (e.newState === State.Running) {
        show({ server: "running", version: starting.initializeResult?.serverInfo?.version });
        if (attached) void laravel.refreshStatus();
      }
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

/** The other extensions that are installed and enabled, among those that double this one. */
function othersEnabled(): Other[] {
  return OTHERS.filter((other) => vscode.extensions.getExtension(other.id) !== undefined);
}

/**
 * Says which other extensions answer the same requests, once per workspace for each
 * set of them. Disabling an extension for a workspace is the user's to do, on its page.
 */
function warnAboutOtherExtensions(context: vscode.ExtensionContext): void {
  if (!vscode.workspace.getConfiguration("phoxite").get<boolean>("warnAboutOtherServers", true)) return;
  const found = othersEnabled();
  const text = conflictMessage(found);
  const seen = "phoxite.otherExtensionsSeen";
  if (!text || context.workspaceState.get<string>(seen) === seenKey(found)) return;
  const showOne = found.length === 1 ? `Show ${found[0].name}` : "Show Them";
  void vscode.window.showWarningMessage(text, showOne, "Not for This Workspace", "Never").then(async (choice) => {
    if (choice === showOne) {
      const picked =
        found.length === 1
          ? found[0]
          : (await vscode.window.showQuickPick(
              found.map((other) => ({ label: other.name, description: other.id, other })),
              { placeHolder: "Open an extension's page: Disable (Workspace) is under its Disable button" },
            ))?.other;
      if (picked) void vscode.commands.executeCommand("extension.open", picked.id);
    } else if (choice === "Not for This Workspace") {
      void context.workspaceState.update(seen, seenKey(found));
    } else if (choice === "Never") {
      void vscode.workspace.getConfiguration("phoxite").update("warnAboutOtherServers", false, true);
    }
  });
}

/** The command of a bare command (a code action that is only one), under this extension's name. */
function rewrittenCommand(command: string): string {
  return withOwnCommand({ command: { command } }).command.command;
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
