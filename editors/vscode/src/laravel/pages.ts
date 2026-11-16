// The Laravel pages as editor tabs: the Eloquent model view (opened from the lens on
// a model class) and the New Eloquent Model form.

import * as vscode from "vscode";
import { FAILED, guarded, registerServerCommand, shown } from "../errors";
import { type Laravel, baseName, count, message, reveal } from "./api";
import { type ItemContext, type PageResources, modelDetails, modelErrorPage, modelViewPage, newModelPage } from "./html";
import {
  type ModelView,
  type NewColumn,
  type NewModelForm,
  type Place,
  chosenColumns,
  columnsError,
  emptyModelForm,
  newModelError,
  newModelSpec,
  shortName,
} from "./model";

/** The id of the model view's pages (`webviewId` in the menus of `package.json`). */
const MODEL_VIEW = "phoxite.modelView";
const NEW_MODEL = "phoxite.newModel";

/** How long the model view waits for a burst of file changes to end. */
const FILES_SETTLE_MS = 300;
/** How often, and how far apart, the view is read again after adding columns. */
const REFRESH_TRIES = 8;
const REFRESH_DELAY_MS = 400;

function panel(context: vscode.ExtensionContext, type: string, title: string): vscode.WebviewPanel {
  const media = vscode.Uri.joinPath(context.extensionUri, "media");
  return vscode.window.createWebviewPanel(type, title, vscode.ViewColumn.Active, {
    enableScripts: true,
    localResourceRoots: [media],
    retainContextWhenHidden: true,
  });
}

function resources(context: vscode.ExtensionContext, webview: vscode.Webview, script: string): PageResources {
  const file = (name: string) => webview.asWebviewUri(vscode.Uri.joinPath(context.extensionUri, "media", name)).toString();
  return { cspSource: webview.cspSource, style: file("laravel.css"), script: file(script) };
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** The open model views, by class: a model's lens shows its view again. */
const modelViews = new Map<string, vscode.WebviewPanel>();

/** Opens the model view of class `cls`. */
export async function openModelView(context: vscode.ExtensionContext, laravel: Laravel, cls: string): Promise<void> {
  const open = modelViews.get(cls);
  if (open) {
    open.reveal();
    return;
  }
  const view = panel(context, MODEL_VIEW, `Model ${shortName(cls)}`);
  const page = resources(context, view.webview, "modelView.js");
  modelViews.set(cls, view);
  let shown: ModelView | undefined;
  let disposed = false;

  const load = async (first: boolean): Promise<void> => {
    try {
      const model = await laravel.modelView(cls);
      if (disposed) return;
      // The page is written once; after that only what it shows is replaced, so a
      // form being filled in stays as it is.
      if (first || !shown) view.webview.html = modelViewPage(model, page);
      else await view.webview.postMessage({ type: "details", html: modelDetails(model) });
      shown = model;
    } catch (e) {
      if (!disposed && (first || !shown)) view.webview.html = modelErrorPage(cls, message(e), page);
    }
  };

  let timer: ReturnType<typeof setTimeout> | undefined;
  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(() => void load(false), FILES_SETTLE_MS);
  };
  // The model, a migration or a related class changed.
  const watcher = vscode.workspace.createFileSystemWatcher("**/*.php");
  watcher.onDidChange(later);
  watcher.onDidCreate(later);
  watcher.onDidDelete(later);

  const addColumns = async (rows: NewColumn[], fillable: boolean): Promise<void> => {
    if (!shown) return;
    const invalid = columnsError(
      rows,
      shown.columns.map((c) => c.name),
    );
    if (invalid) {
      await view.webview.postMessage({ type: "error", message: invalid });
      return;
    }
    const columns = chosenColumns(rows);
    try {
      await laravel.apply(await laravel.addColumns(shown.class, columns, fillable));
    } catch (e) {
      await view.webview.postMessage({ type: "error", message: message(e) });
      return;
    }
    await view.webview.postMessage({ type: "added" });
    void vscode.window.showInformationMessage(`Added ${count(columns.length, "column")} to ${shown.table}.`);
    // The server sees the migration once the file watcher reports it.
    for (let attempt = 0; attempt < REFRESH_TRIES && !disposed; attempt++) {
      await load(false);
      if (columns.every((c) => shown?.columns.some((known) => known.name === c.name))) break;
      await sleep(REFRESH_DELAY_MS);
    }
  };

  view.webview.onDidReceiveMessage((m: { type?: string; place?: Place; columns?: NewColumn[]; fillable?: boolean }) => {
    if (m.type === "goto" && m.place) void reveal(m.place, vscode.ViewColumn.Beside);
    else if (m.type === "addColumns") void addColumns(Array.isArray(m.columns) ? m.columns : [], !!m.fillable);
  });
  view.onDidDispose(() => {
    disposed = true;
    clearTimeout(timer);
    watcher.dispose();
    modelViews.delete(cls);
  });
  await load(true);
}

/** Opens the place of a model view item beside the view and runs editor command `command` there. */
async function atItem(item: ItemContext | undefined, command: string): Promise<void> {
  if (!item?.place) return;
  await reveal(item.place, vscode.ViewColumn.Beside);
  await vscode.commands.executeCommand(command);
}

/** "Open Model View…": picks one of the project's models. */
async function pickModel(context: vscode.ExtensionContext, laravel: Laravel): Promise<void> {
  const ctx = await shown(laravel.generationContext());
  if (ctx === FAILED) return;
  if (!ctx?.models.length) {
    void vscode.window.showInformationMessage("No Eloquent models were found in this workspace.");
    return;
  }
  const picked = await vscode.window.showQuickPick(
    ctx.models.map((model) => ({ label: shortName(model), description: model, model })),
    { placeHolder: "Eloquent model", matchOnDescription: true },
  );
  if (picked) await openModelView(context, laravel, picked.model);
}

/** Opens the New Eloquent Model form. */
export async function openNewModel(context: vscode.ExtensionContext, laravel: Laravel): Promise<void> {
  const ctx = await shown(laravel.generationContext());
  if (ctx === FAILED) return;
  if (!ctx) {
    void vscode.window.showInformationMessage("Laravel generators need a Laravel project in this workspace.");
    return;
  }
  const view = panel(context, NEW_MODEL, "New Eloquent Model");
  view.webview.html = newModelPage(ctx, emptyModelForm(ctx.defaultModule ?? ""), resources(context, view.webview, "newModel.js"));
  const create = async (form: NewModelForm): Promise<void> => {
    const invalid = newModelError(form);
    if (invalid) {
      await view.webview.postMessage({ type: "error", message: invalid });
      return;
    }
    let created: vscode.Uri[];
    try {
      created = await laravel.apply(await laravel.newModel(newModelSpec(form)));
    } catch (e) {
      await view.webview.postMessage({ type: "error", message: message(e) });
      return;
    }
    view.dispose();
    if (created[0]) await vscode.window.showTextDocument(created[0]);
    void vscode.window.showInformationMessage(
      `Created ${count(created.length, "file")} for ${newModelSpec(form).name}: ${created.map(baseName).join(", ")}.`,
    );
  };
  view.webview.onDidReceiveMessage((m: { type?: string; form?: NewModelForm }) => {
    if (m.type === "cancel") view.dispose();
    else if (m.type === "create" && m.form) void create(m.form);
  });
}

export function registerPages(context: vscode.ExtensionContext, laravel: Laravel): void {
  context.subscriptions.push(
    // The lens on a model class: its argument is the class.
    registerServerCommand("laravel.modelView", (cls: unknown) => {
      if (typeof cls === "string" && cls.trim()) return openModelView(context, laravel, cls.trim());
      return pickModel(context, laravel);
    }),
    vscode.commands.registerCommand(
      "phoxite.laravel.modelView",
      guarded(() => pickModel(context, laravel)),
    ),
    vscode.commands.registerCommand("phoxite.laravel.modelView.findUsages", (item?: ItemContext) =>
      atItem(item, "references-view.findReferences"),
    ),
    vscode.commands.registerCommand("phoxite.laravel.modelView.rename", (item?: ItemContext) =>
      atItem(item, "editor.action.rename"),
    ),
    vscode.commands.registerCommand(
      "phoxite.laravel.newModel",
      guarded(() => openNewModel(context, laravel)),
    ),
    { dispose: () => [...modelViews.values()].forEach((view) => view.dispose()) },
  );
}
