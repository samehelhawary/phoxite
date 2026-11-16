// The routes of the Laravel project: the "Laravel Routes" view (the route files
// with their routes) and "Go to Route…".

import * as path from "node:path";
import * as vscode from "vscode";
import { type Route, openable, routeLabel } from "../routes";
import { type Laravel, message, reveal } from "./api";

/** How long the view waits for a burst of saves to end before reading the routes again. */
const SETTLE_MS = 400;

type Node = { file: string; routes: Route[] } | { route: Route };

/** A route file's path as the view shows it: relative to its workspace folder. */
function fileLabel(file: string): string {
  return vscode.workspace.asRelativePath(file, (vscode.workspace.workspaceFolders?.length ?? 0) > 1);
}

/** What a route's tooltip says: its action and middleware. */
function routeTooltip(route: Route): vscode.MarkdownString {
  const lines = [`**${routeLabel(route)}**`, "", `Action: \`${route.action}\``];
  if (route.name) lines.push("", `Name: \`${route.name}\``);
  if (route.middleware.length) lines.push("", `Middleware: ${route.middleware.map((m) => `\`${m}\``).join(", ")}`);
  if (route.domain) lines.push("", `Domain: \`${route.domain}\``);
  return new vscode.MarkdownString(lines.join("\n"));
}

class RoutesProvider implements vscode.TreeDataProvider<Node> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private files: Array<{ file: string; routes: Route[] }> | undefined;
  /** Told why the routes couldn't be read, or that they could (`undefined`). */
  onProblem: (problem: string | undefined) => void = () => {};

  constructor(private readonly laravel: Laravel) {}

  refresh(): void {
    this.files = undefined;
    this.changed.fire();
  }

  getTreeItem(node: Node): vscode.TreeItem {
    if ("file" in node) {
      const item = new vscode.TreeItem(fileLabel(node.file), vscode.TreeItemCollapsibleState.Expanded);
      item.description = String(node.routes.length);
      item.resourceUri = vscode.Uri.file(node.file);
      item.iconPath = vscode.ThemeIcon.File;
      item.contextValue = "routeFile";
      return item;
    }
    const { route } = node;
    const item = new vscode.TreeItem(routeLabel(route));
    item.description = route.name ?? route.action;
    item.tooltip = routeTooltip(route);
    item.iconPath = new vscode.ThemeIcon("symbol-event");
    item.contextValue = "route";
    item.command = { command: "phoxite.laravel.route.goto", title: "Go to Declaration", arguments: [node] };
    return item;
  }

  async getChildren(node?: Node): Promise<Node[]> {
    if (node) return "file" in node ? node.routes.map((route) => ({ route })) : [];
    if (!this.laravel.isActive) return [];
    if (!this.files) {
      // An empty view for a server that is down would read as "this project has no routes".
      let routes: Route[];
      try {
        routes = await this.laravel.routes();
        this.onProblem(undefined);
      } catch (e) {
        this.onProblem(message(e));
        return [];
      }
      const byFile = new Map<string, Route[]>();
      for (const route of routes) {
        const key = path.normalize(route.path);
        byFile.set(key, [...(byFile.get(key) ?? []), route]);
      }
      this.files = [...byFile].map(([file, routes]) => ({ file, routes }));
    }
    return this.files;
  }
}

/** The route a command got: a node of the view. */
const routeOf = (arg: unknown): Route | undefined =>
  arg && typeof arg === "object" && "route" in arg ? (arg as { route: Route }).route : undefined;

async function openInBrowser(route: Route): Promise<void> {
  if (openable(route.url)) {
    await vscode.env.openExternal(vscode.Uri.parse(route.url));
    return;
  }
  const url = await vscode.window.showInputBox({ prompt: "Fill in the route's parameters", value: route.url });
  if (url) await vscode.env.openExternal(vscode.Uri.parse(url));
}

/** "Go to Route…": every route of the project, searched by method, URI, name or action. */
async function gotoRoute(laravel: Laravel): Promise<void> {
  let routes: Route[];
  try {
    routes = await laravel.routes();
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
    return;
  }
  if (!routes.length) {
    void vscode.window.showInformationMessage("No Laravel routes were found in this workspace.");
    return;
  }
  const picked = await vscode.window.showQuickPick(
    routes.map((route) => ({
      label: routeLabel(route),
      description: route.name ?? undefined,
      detail: route.action,
      route,
    })),
    { placeHolder: "Search routes by method, URI, name or action", matchOnDescription: true, matchOnDetail: true },
  );
  if (picked) await reveal(picked.route);
}

export function registerRoutes(context: vscode.ExtensionContext, laravel: Laravel): void {
  const provider = new RoutesProvider(laravel);
  const view = vscode.window.createTreeView("phoxite.laravel.routes", { treeDataProvider: provider });
  provider.onProblem = (problem) => {
    view.message = problem ? `The routes couldn't be read: ${problem}` : undefined;
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(() => provider.refresh(), SETTLE_MS);
  };
  const withRoute = (run: (route: Route) => unknown) => (arg: unknown) => {
    const route = routeOf(arg);
    if (route) return run(route);
    return undefined;
  };
  context.subscriptions.push(
    view,
    { dispose: () => clearTimeout(timer) },
    laravel.onDidChange(() => provider.refresh()),
    // Routes are read from the route files and the providers registering them.
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "php" && view.visible) later();
    }),
    view.onDidChangeVisibility((e) => {
      if (e.visible) later();
    }),
    vscode.commands.registerCommand("phoxite.laravel.routes.refresh", () => provider.refresh()),
    vscode.commands.registerCommand("phoxite.laravel.routes.goto", () => gotoRoute(laravel)),
    vscode.commands.registerCommand(
      "phoxite.laravel.route.goto",
      withRoute((route) => reveal(route)),
    ),
    vscode.commands.registerCommand(
      "phoxite.laravel.route.copyUrl",
      withRoute(async (route) => {
        await vscode.env.clipboard.writeText(route.url);
        vscode.window.setStatusBarMessage(`Copied ${route.url}`, 3000);
      }),
    ),
    vscode.commands.registerCommand("phoxite.laravel.route.open", withRoute(openInBrowser)),
    vscode.commands.registerCommand(
      "phoxite.laravel.route.copyRequest",
      withRoute(async (route) => {
        await vscode.env.clipboard.writeText(route.request);
        vscode.window.setStatusBarMessage("Copied the HTTP request", 3000);
      }),
    ),
  );
}
