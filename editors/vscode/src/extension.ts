// The VS Code client of phoxite-lsp.

import * as fs from "node:fs";
import * as path from "node:path";
import * as vscode from "vscode";
import {
  type Executable,
  LanguageClient,
  type LanguageClientOptions,
  RevealOutputChannelOn,
  type ServerOptions,
} from "vscode-languageclient/node";
import { Laravel } from "./laravel/api";
import { registerCommands } from "./laravel/commands";
import { registerPages } from "./laravel/pages";
import { registerRoutes } from "./laravel/routesView";
import { Refactorings, registerRefactorings } from "./refactor/commands";
import { openable, parseRoutes, routePicks } from "./routes";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel | undefined;
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
    vscode.commands.registerCommand("phoxite.rescan", () =>
      client?.sendRequest("workspace/executeCommand", { command: "phoxite.server.rescan", arguments: [] }),
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
    }),
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
  await laravel.reset();
  await client?.stop();
  client = undefined;
}

/** The server executable: the setting, else the one bundled for this platform. */
function serverPath(context: vscode.ExtensionContext): string | undefined {
  const configured = vscode.workspace.getConfiguration("phoxite").get<string>("server.path")?.trim();
  if (configured) return configured;
  const name = process.platform === "win32" ? "phoxite-lsp.exe" : "phoxite-lsp";
  const bundled = path.join(context.extensionPath, "server", name);
  return fs.existsSync(bundled) ? bundled : undefined;
}

function settings(): unknown {
  return vscode.workspace.getConfiguration("phoxite");
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const command = serverPath(context);
  if (!command) {
    const choice = await vscode.window.showErrorMessage(
      "Phoxite: no language server for this platform. Set `phoxite.server.path` to a phoxite-lsp executable.",
      "Open Settings",
    );
    if (choice) void vscode.commands.executeCommand("workbench.action.openSettings", "phoxite.server.path");
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
  client = new LanguageClient("phoxite", "Phoxite", serverOptions, clientOptions);
  // This client has the dialogs of the refactorings that ask something first, so
  // the server offers them (`refactor/commands.ts`).
  client.registerFeature({
    fillClientCapabilities(capabilities) {
      const experimental = (capabilities.experimental ?? {}) as Record<string, unknown>;
      capabilities.experimental = { ...experimental, phoxite: { dialogs: true } };
    },
    initialize() {},
    getState: () => ({ kind: "static" }),
    clear() {},
  });
  try {
    await client.start();
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: the language server didn't start: ${String(e)}`);
    return;
  }
  // Not awaited: it asks a few times while the server reads the project.
  void laravel.refreshStatus();
}

async function restart(context: vscode.ExtensionContext): Promise<void> {
  await laravel.reset();
  await client?.stop();
  client = undefined;
  await start(context);
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
