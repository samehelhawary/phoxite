// The routes of the Laravel projects: the "Laravel Routes" view (the route files with
// their routes, under their project when the workspace has several) and "Go to Route…".

import * as path from "node:path";
import * as vscode from "vscode";
import { type Route, type RoutesState, openable, routeLabel, routeMatches, routesState } from "../routes";
import { type Laravel, folderName, message, reveal } from "./api";

/** How long the view waits for a burst of changes to end before reading the routes again. */
const SETTLE_MS = 400;

/** The context keys the view's title actions and its empty texts show on. */
const STATE_KEY = "phoxite.laravel.routes.state";
const FILTER_KEY = "phoxite.laravel.routes.filter";

type FileNode = { file: string; routes: Route[] };
type Node = { folder: string; files: FileNode[] } | FileNode | { route: Route };

/** The routes of one project; `folder` (a URI) is there when the server says which it is. */
interface ProjectRoutes {
  folder?: string;
  routes: Route[];
}

/** A route file's path as the view shows it: relative to its workspace folder. */
function fileLabel(file: string): string {
  return vscode.workspace.asRelativePath(file, false);
}

/** What a route's tooltip says: its action and middleware. */
function routeTooltip(route: Route): vscode.MarkdownString {
  const lines = [`**${routeLabel(route)}**`, "", `Action: \`${route.action}\``];
  if (route.name) lines.push("", `Name: \`${route.name}\``);
  if (route.middleware.length) lines.push("", `Middleware: ${route.middleware.map((m) => `\`${m}\``).join(", ")}`);
  if (route.domain) lines.push("", `Domain: \`${route.domain}\``);
  return new vscode.MarkdownString(lines.join("\n"));
}

/** The routes of every Laravel project of the workspace. */
async function readRoutes(laravel: Laravel): Promise<ProjectRoutes[]> {
  // One project, or a server that doesn't say where the projects are: the project it picks.
  if (laravel.folders.length < 2) return [{ folder: laravel.folders[0], routes: await laravel.routes() }];
  return Promise.all(laravel.folders.map(async (folder) => ({ folder, routes: await laravel.in(folder).routes() })));
}

function byFile(routes: Route[]): FileNode[] {
  const files = new Map<string, Route[]>();
  for (const route of routes) {
    const key = path.normalize(route.path);
    files.set(key, [...(files.get(key) ?? []), route]);
  }
  return [...files].map(([file, routes]) => ({ file, routes }));
}

class RoutesProvider implements vscode.TreeDataProvider<Node> {
  private readonly changed = new vscode.EventEmitter<void>();
  readonly onDidChangeTreeData = this.changed.event;
  private projects: ProjectRoutes[] | undefined;
  private filter = "";
  /** Told what the view has: routes, none, none passing the filter, or why they couldn't be read. */
  onState: (state: RoutesState, problem?: string) => void = () => {};

  constructor(private readonly laravel: Laravel) {}

  /** Reads the routes again. */
  refresh(): void {
    this.projects = undefined;
    this.changed.fire();
  }

  get filtered(): string {
    return this.filter;
  }

  /** Shows only the routes that match `filter`; the routes are not read again. */
  setFilter(filter: string): void {
    this.filter = filter.trim();
    this.changed.fire();
  }

  getTreeItem(node: Node): vscode.TreeItem {
    if ("folder" in node) {
      const item = new vscode.TreeItem(folderName(node.folder), vscode.TreeItemCollapsibleState.Expanded);
      item.description = String(node.files.reduce((n, file) => n + file.routes.length, 0));
      item.iconPath = vscode.ThemeIcon.Folder;
      item.resourceUri = vscode.Uri.parse(node.folder);
      item.contextValue = "routeProject";
      return item;
    }
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
    if (node) {
      if ("folder" in node) return node.files;
      return "file" in node ? node.routes.map((route) => ({ route })) : [];
    }
    if (!this.laravel.isActive) return [];
    if (!this.projects) {
      // An empty view for a server that is down would read as "this project has no routes".
      try {
        this.projects = await readRoutes(this.laravel);
      } catch (e) {
        this.onState("failed", message(e));
        return [];
      }
    }
    const found = this.projects.reduce((n, project) => n + project.routes.length, 0);
    const kept = this.projects
      .map((project) => ({ ...project, routes: project.routes.filter((route) => routeMatches(route, this.filter)) }))
      .filter((project) => project.routes.length);
    this.onState(routesState(found, kept.reduce((n, project) => n + project.routes.length, 0)));
    // The projects are named when there are several of them.
    if (this.projects.length > 1) {
      return kept.map((project) => ({ folder: project.folder ?? "", files: byFile(project.routes) }));
    }
    return kept.flatMap((project) => byFile(project.routes));
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

/** "Go to Route…": every route of the workspace, searched by method, URI, name or action. */
async function gotoRoute(laravel: Laravel): Promise<void> {
  let projects: ProjectRoutes[];
  try {
    projects = await readRoutes(laravel);
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
    return;
  }
  const several = projects.length > 1;
  const items = projects.flatMap((project) =>
    project.routes.map((route) => ({
      label: routeLabel(route),
      description: [route.name, several && project.folder ? folderName(project.folder) : undefined].filter(Boolean).join(" · "),
      detail: route.action,
      route,
    })),
  );
  if (!items.length) {
    void vscode.window.showInformationMessage("No Laravel routes were found in this workspace.");
    return;
  }
  const picked = await vscode.window.showQuickPick(items, {
    placeHolder: "Search routes by method, URI, name or action",
    matchOnDescription: true,
    matchOnDetail: true,
  });
  if (picked) await reveal(picked.route);
}

export function registerRoutes(context: vscode.ExtensionContext, laravel: Laravel): void {
  const provider = new RoutesProvider(laravel);
  const view = vscode.window.createTreeView("phoxite.laravel.routes", { treeDataProvider: provider, showCollapseAll: true });
  provider.onState = (state, problem) => {
    // The empty view says which of the three it is (`viewsWelcome` in `package.json`).
    void vscode.commands.executeCommand("setContext", STATE_KEY, state);
    view.message = state === "failed" && problem ? problem : undefined;
  };
  const showFilter = () => {
    view.description = provider.filtered ? `filter: ${provider.filtered}` : undefined;
    void vscode.commands.executeCommand("setContext", FILTER_KEY, provider.filtered !== "");
  };
  let timer: ReturnType<typeof setTimeout> | undefined;
  const later = () => {
    if (!view.visible) return;
    clearTimeout(timer);
    timer = setTimeout(() => provider.refresh(), SETTLE_MS);
  };
  const withRoute = (run: (route: Route) => unknown) => (arg: unknown) => {
    const route = routeOf(arg);
    if (route) return run(route);
    return undefined;
  };
  // Routes are read from the route files and the providers registering them: a saved,
  // a new and a deleted PHP file can each change them.
  const files = vscode.workspace.createFileSystemWatcher("**/*.php", false, true, false);
  context.subscriptions.push(
    view,
    files,
    { dispose: () => clearTimeout(timer) },
    laravel.onDidChange(() => provider.refresh()),
    files.onDidCreate(later),
    files.onDidDelete(later),
    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "php") later();
    }),
    view.onDidChangeVisibility(later),
    vscode.commands.registerCommand("phoxite.laravel.routes.refresh", () => provider.refresh()),
    vscode.commands.registerCommand("phoxite.laravel.routes.goto", () => gotoRoute(laravel)),
    vscode.commands.registerCommand("phoxite.laravel.routes.filter", async (given?: unknown) => {
      // A caller (a keybinding's `args`, a test) may name the filter.
      const filter =
        typeof given === "string"
          ? given
          : await vscode.window.showInputBox({
              title: "Filter Routes",
              prompt: "Words of the method, URI, name or action; empty shows every route",
              value: provider.filtered,
            });
      if (filter === undefined) return;
      provider.setFilter(filter);
      showFilter();
    }),
    vscode.commands.registerCommand("phoxite.laravel.routes.clearFilter", () => {
      provider.setFilter("");
      showFilter();
    }),
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
