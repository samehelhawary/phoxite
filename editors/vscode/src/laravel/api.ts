// The server's `phoxite/laravel/*` requests, and applying the edits they answer.

import * as vscode from "vscode";
import type { LanguageClient, WorkspaceEdit as ProtocolEdit } from "vscode-languageclient/node";
import { message, running } from "../errors";
import type { Route } from "../routes";
import {
  type Applied,
  type ArtisanCommand,
  type GenerationContext,
  type ModelView,
  type NewClassSpec,
  type NewColumn,
  type NewModelSpec,
  type Place,
  type SchemaStatus,
  type Template,
  chosenFolder,
  migrationTimestamp,
} from "./model";

/** The context key the Laravel views and menus show on. */
const CONTEXT_KEY = "phoxite.laravel";

/**
 * How often, and how far apart, a server that doesn't say so itself is asked whether the
 * project is a Laravel one (servers before 0.3 answer before they have read the project).
 */
const STATUS_TRIES = 6;
const STATUS_DELAY_MS = 1500;

/** The notification of a server that says when a folder is (or stops being) a Laravel project. */
const STATUS_CHANGED = "phoxite/laravel/statusChanged";

/** A workspace folder's URI as VS Code writes it, whoever wrote `uri`. */
function folderUri(uri: string): string {
  const parsed = vscode.Uri.parse(uri);
  return (vscode.workspace.getWorkspaceFolder(parsed)?.uri ?? parsed).toString();
}

/** The name of workspace folder `folder` (a URI). */
export function folderName(folder: string): string {
  const uri = vscode.Uri.parse(folder);
  return vscode.workspace.getWorkspaceFolder(uri)?.name ?? baseName(uri);
}

/**
 * The server's requests about one Laravel project: the one in workspace folder
 * `folder`, or the one the server picks when there is none.
 */
export class Project {
  constructor(
    protected readonly client: () => LanguageClient | undefined,
    private readonly fixed?: string,
  ) {}

  /** The workspace folder (a URI) the requests are about. */
  get folder(): string | undefined {
    return this.fixed;
  }

  protected async request<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    return running(this.client()).sendRequest<T>(`phoxite/laravel/${method}`, { folder: this.folder, ...params });
  }

  routes(query = ""): Promise<Route[]> {
    return this.request("routes", { query });
  }

  modelView(cls: string): Promise<ModelView> {
    return this.request("modelView", { class: cls });
  }

  addColumns(cls: string, columns: NewColumn[], fillable: boolean): Promise<Applied> {
    return this.request("addColumns", { class: cls, columns, fillable, timestamp: migrationTimestamp(new Date()) });
  }

  artisanCommands(): Promise<ArtisanCommand[] | null> {
    return this.request("artisanCommands");
  }

  generationContext(): Promise<GenerationContext | null> {
    return this.request("generationContext");
  }

  newModel(spec: NewModelSpec): Promise<Applied> {
    return this.request("newModel", { spec, timestamp: migrationTimestamp(new Date()) });
  }

  generateClass(spec: NewClassSpec): Promise<Applied> {
    return this.request("generateClass", { spec, timestamp: migrationTimestamp(new Date()) });
  }

  createTranslation(key: string, texts: Array<{ locale: string; text: string }>): Promise<Applied> {
    return this.request("createTranslation", { key, texts });
  }

  templates(): Promise<Template[]> {
    return this.request("templates");
  }

  /** Has the server read the database of `phoxite.laravel.database` again. */
  refreshSchema(): Promise<SchemaStatus | null> {
    return this.request("refreshSchema");
  }

  /**
   * Applies an edit of the server and saves the files it touched (those that had no
   * unsaved changes before): the server indexes what is on disk. Answers the files
   * the edit created.
   */
  async apply(applied: Applied): Promise<vscode.Uri[]> {
    const client = running(this.client());
    const edit = await client.protocol2CodeConverter.asWorkspaceEdit(applied.edit as ProtocolEdit);
    if (!edit) throw new Error("The server answered no edit");
    const touched = edit.entries().map(([uri]) => uri.toString());
    const dirtyBefore = new Set(vscode.workspace.textDocuments.filter((d) => d.isDirty).map((d) => d.uri.toString()));
    if (!(await vscode.workspace.applyEdit(edit, { isRefactoring: true }))) {
      throw new Error("The edit couldn't be applied");
    }
    for (const doc of vscode.workspace.textDocuments) {
      const uri = doc.uri.toString();
      if (doc.isDirty && touched.includes(uri) && !dirtyBefore.has(uri)) await doc.save();
    }
    return applied.created.map((uri) => vscode.Uri.parse(uri));
  }
}

/**
 * The Laravel projects of the workspace, as far as the server said. Its own requests
 * are about the project of the active editor; `in` gives those of one folder, for
 * what outlives the active editor (a panel) or was asked about another (the Explorer).
 */
export class Laravel extends Project {
  private active = false;
  /** The workspace folders that are Laravel projects; empty when the server doesn't say. */
  private known: string[] = [];
  private check = 0;
  /** The server tells when the status changes, so asking once is enough. */
  private told = false;
  private readonly changed = new vscode.EventEmitter<boolean>();
  /** The workspace turned out to be a Laravel project, or stopped being one. */
  readonly onDidChange = this.changed.event;
  private named: string[] = [];
  private readonly renamed = new vscode.EventEmitter<readonly string[]>();
  /** The frameworks of the open projects changed: their names, as the server says them. */
  readonly onDidChangeFrameworks = this.renamed.event;

  /** The frameworks of the open projects (`Laravel`, `WordPress`); empty when the server doesn't say. */
  get frameworks(): readonly string[] {
    return this.named;
  }

  private setFrameworks(names: unknown): void {
    const next = Array.isArray(names) ? names.filter((name): name is string => typeof name === "string") : [];
    if (next.join("\n") === this.named.join("\n")) return;
    this.named = next;
    this.renamed.fire(next);
  }

  constructor(client: () => LanguageClient | undefined) {
    super(client);
  }

  /** A Laravel project is open, as far as the server said. */
  get isActive(): boolean {
    return this.active;
  }

  /** The workspace folders (URIs) that are Laravel projects; empty when the server doesn't say. */
  get folders(): readonly string[] {
    return this.known;
  }

  /** The Laravel project of the active editor, else the first one. */
  override get folder(): string | undefined {
    const uri = vscode.window.activeTextEditor?.document.uri;
    const folder = uri && uri.scheme === "file" ? vscode.workspace.getWorkspaceFolder(uri) : undefined;
    return chosenFolder(folder?.uri.toString(), this.known);
  }

  /** The requests about the project in `folder`; without one, the project of this moment. */
  in(folder?: string): Project {
    return new Project(this.client, folder ? chosenFolder(folderUri(folder), this.known) : this.folder);
  }

  /** The project file `uri` is in. */
  of(uri: vscode.Uri): Project {
    return this.in(vscode.workspace.getWorkspaceFolder(uri)?.uri.toString());
  }

  /** Sets the context key the views and menus show on, and tells who listens. */
  private async set(found: boolean, folders: string[] = []): Promise<void> {
    const same = found === this.active && folders.join("\n") === this.known.join("\n");
    this.known = folders;
    if (same) return;
    if (found !== this.active) {
      this.active = found;
      await vscode.commands.executeCommand("setContext", CONTEXT_KEY, found);
    }
    this.changed.fire(found);
  }

  /**
   * A server started: listens to what it says about Laravel projects, and asks once
   * for what it knows now. The answer of the listener stays while the client lives.
   */
  attach(client: LanguageClient): vscode.Disposable {
    const experimental = client.initializeResult?.capabilities.experimental as
      | { phoxite?: { laravelStatus?: boolean } }
      | undefined;
    this.told = experimental?.phoxite?.laravelStatus === true;
    type Told = { laravel?: boolean; folders?: string[]; frameworks?: string[] };
    const listener = client.onNotification(STATUS_CHANGED, (status: Told) => {
      // What the server says now is newer than the answer of a question still on its way.
      this.check++;
      this.setFrameworks(status?.frameworks);
      const folders = Array.isArray(status?.folders) ? status.folders.map(folderUri) : [];
      void this.set(status?.laravel === true, folders);
    });
    void this.refreshStatus();
    return listener;
  }

  /**
   * Asks the server whether a Laravel project is open. A server that tells when that
   * changes is asked once; an older one a few times, as it may answer before it has
   * read the project.
   */
  async refreshStatus(): Promise<boolean> {
    const check = ++this.check;
    const tries = this.told ? 1 : STATUS_TRIES;
    let found = false;
    for (let attempt = 0; attempt < tries && check === this.check; attempt++) {
      found = await this.request<{ laravel: boolean; frameworks?: string[] }>("status").then(
        (status) => {
          if (check === this.check) this.setFrameworks(status.frameworks);
          return status.laravel;
        },
        () => false,
      );
      if (found || attempt + 1 === tries) break;
      await new Promise((resolve) => setTimeout(resolve, STATUS_DELAY_MS));
    }
    if (check !== this.check) return this.active;
    // The folders are what the server last said; a server that doesn't say has none.
    await this.set(found, found ? this.known : []);
    return found;
  }

  /** The server stopped: nothing is known until it runs again. */
  async reset(): Promise<void> {
    this.check++;
    this.setFrameworks([]);
    await this.set(false);
  }

  dispose(): void {
    this.changed.dispose();
  }
}

/** The editor range of an engine range. */
export function toRange(range: Place["range"]): vscode.Range {
  return new vscode.Range(range.start.line, range.start.col, range.end.line, range.end.col);
}

/** Opens the file of `place` with its range selected. */
export async function reveal(place: Place, column?: vscode.ViewColumn): Promise<vscode.TextEditor> {
  return vscode.window.showTextDocument(vscode.Uri.file(place.path), {
    selection: toRange(place.range),
    viewColumn: column,
  });
}

export { message };

/** `2 files` / `1 file`. */
export const count = (n: number, noun: string) => `${n} ${noun}${n === 1 ? "" : "s"}`;

/** The file name of a URI. */
export const baseName = (uri: vscode.Uri) => uri.path.slice(uri.path.lastIndexOf("/") + 1);
