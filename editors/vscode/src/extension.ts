// The VS Code client of php-engine-lsp.

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
import { openable, parseRoutes, routePicks } from "./routes";

let client: LanguageClient | undefined;
let output: vscode.OutputChannel | undefined;

/** Other PHP language servers that would answer the same requests. */
const OTHER_SERVERS = ["bmewburn.vscode-intelephense-client", "zobo.php-intellisense", "phpactor.vscode-phpactor"];

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  output = vscode.window.createOutputChannel("PHP Engine", { log: true });
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("phpEngine.restart", () => restart(context)),
    vscode.commands.registerCommand("phpEngine.showOutput", () => output?.show()),
    vscode.commands.registerCommand("phpEngine.rescan", () =>
      client?.sendRequest("workspace/executeCommand", { command: "phpEngine.rescan", arguments: [] }),
    ),
    vscode.commands.registerCommand("phpEngine.openLocation", openLocation),
    vscode.commands.registerCommand("laravel.routeActions", routeActions),
    vscode.commands.registerCommand("laravel.modelView", (cls: unknown) =>
      vscode.window.showInformationMessage(
        `The model view of ${String(cls)} isn't available in VS Code yet. Its columns, relations and scopes show in completion and hover.`,
      ),
    ),
  );
  warnAboutOtherServers();
  await start(context);
}

export async function deactivate(): Promise<void> {
  await client?.stop();
  client = undefined;
}

/** The server executable: the setting, else the one bundled for this platform. */
function serverPath(context: vscode.ExtensionContext): string | undefined {
  const configured = vscode.workspace.getConfiguration("phpEngine").get<string>("server.path")?.trim();
  if (configured) return configured;
  const name = process.platform === "win32" ? "php-engine-lsp.exe" : "php-engine-lsp";
  const bundled = path.join(context.extensionPath, "server", name);
  return fs.existsSync(bundled) ? bundled : undefined;
}

function settings(): unknown {
  return vscode.workspace.getConfiguration("phpEngine");
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const command = serverPath(context);
  if (!command) {
    const choice = await vscode.window.showErrorMessage(
      "PHP Engine: no language server for this platform. Set `phpEngine.server.path` to a php-engine-lsp executable.",
      "Open Settings",
    );
    if (choice) void vscode.commands.executeCommand("workbench.action.openSettings", "phpEngine.server.path");
    return;
  }
  const run: Executable = { command, args: ["--stdio"], options: { env: { ...process.env } } };
  const serverOptions: ServerOptions = { run, debug: { ...run, options: { env: { ...process.env, PHP_ENGINE_LOG: "debug" } } } };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "php" },
      { scheme: "file", language: "blade" },
      // Environment keys and JSON translations: usages and rename.
      { scheme: "file", pattern: "**/.env*" },
      { scheme: "file", pattern: "**/lang/**/*.json" },
    ],
    synchronize: {
      configurationSection: "phpEngine",
      fileEvents: [
        vscode.workspace.createFileSystemWatcher("**/*.php"),
        vscode.workspace.createFileSystemWatcher("**/composer.{json,lock}"),
        vscode.workspace.createFileSystemWatcher("**/vendor/composer/installed.json"),
        vscode.workspace.createFileSystemWatcher("**/.env*"),
        vscode.workspace.createFileSystemWatcher("**/lang/**/*.json"),
        vscode.workspace.createFileSystemWatcher("**/ide.json"),
        vscode.workspace.createFileSystemWatcher("**/.phpstorm.meta.php"),
      ],
    },
    initializationOptions: {
      cacheDir: path.join(context.globalStorageUri.fsPath, "index"),
      settings: settings(),
    },
    outputChannel: output,
    revealOutputChannelOn: RevealOutputChannelOn.Never,
  };
  client = new LanguageClient("phpEngine", "PHP Engine", serverOptions, clientOptions);
  try {
    await client.start();
  } catch (e) {
    void vscode.window.showErrorMessage(`PHP Engine: the language server didn't start: ${String(e)}`);
  }
}

async function restart(context: vscode.ExtensionContext): Promise<void> {
  await client?.stop();
  client = undefined;
  await start(context);
}

function warnAboutOtherServers(): void {
  if (!vscode.workspace.getConfiguration("phpEngine").get<boolean>("warnAboutOtherServers", true)) return;
  const others = OTHER_SERVERS.map((id) => vscode.extensions.getExtension(id)).filter((e) => e !== undefined);
  if (others.length === 0) return;
  const names = others.map((e) => e.packageJSON.displayName ?? e.id).join(", ");
  void vscode.window
    .showWarningMessage(
      `PHP Engine: ${names} is enabled too, so completions and problems show twice. Disable it for this workspace?`,
      "Show Extension",
      "Don't Ask Again",
    )
    .then((choice) => {
      if (choice === "Show Extension") {
        void vscode.commands.executeCommand("workbench.extensions.search", `@id:${others[0].id}`);
      } else if (choice === "Don't Ask Again") {
        void vscode.workspace.getConfiguration("phpEngine").update("warnAboutOtherServers", false, true);
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
