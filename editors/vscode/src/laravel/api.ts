// The server's `phoxite/laravel/*` requests, and applying the edits they answer.

import * as vscode from "vscode";
import type { LanguageClient, WorkspaceEdit as ProtocolEdit } from "vscode-languageclient/node";
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
  migrationTimestamp,
} from "./model";

/** The context key the Laravel views and menus show on. */
const CONTEXT_KEY = "phoxite.laravel";

/** How often, and how far apart, a started server is asked whether the project is a Laravel one. */
const STATUS_TRIES = 6;
const STATUS_DELAY_MS = 1500;

export class Laravel {
  private active = false;
  private check = 0;
  private readonly changed = new vscode.EventEmitter<boolean>();
  /** The workspace turned out to be a Laravel project, or stopped being one. */
  readonly onDidChange = this.changed.event;

  constructor(private readonly client: () => LanguageClient | undefined) {}

  /** A Laravel project is open, as far as the server said. */
  get isActive(): boolean {
    return this.active;
  }

  /** The folder the requests are about: the active editor's, when it is in one. */
  private folder(): string | undefined {
    const uri = vscode.window.activeTextEditor?.document.uri;
    const folder = uri && uri.scheme === "file" ? vscode.workspace.getWorkspaceFolder(uri) : undefined;
    return folder?.uri.toString();
  }

  private async request<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
    const client = this.client();
    if (!client?.isRunning()) throw new Error("The Phoxite language server isn't running");
    return client.sendRequest<T>(`phoxite/laravel/${method}`, { folder: this.folder(), ...params });
  }

  /**
   * Asks the server whether a Laravel project is open and sets the context key the
   * views and menus show on. A server that just started is asked a few times: it may
   * answer before it has read the project.
   */
  async refreshStatus(): Promise<boolean> {
    const check = ++this.check;
    let found = false;
    for (let attempt = 0; attempt < STATUS_TRIES && check === this.check; attempt++) {
      found = await this.request<{ laravel: boolean }>("status").then(
        (status) => status.laravel,
        () => false,
      );
      if (found) break;
      await new Promise((resolve) => setTimeout(resolve, STATUS_DELAY_MS));
    }
    if (check !== this.check) return this.active;
    if (found !== this.active) {
      this.active = found;
      await vscode.commands.executeCommand("setContext", CONTEXT_KEY, found);
      this.changed.fire(found);
    }
    return found;
  }

  /** The server stopped: nothing is known until it runs again. */
  async reset(): Promise<void> {
    this.check++;
    if (!this.active) return;
    this.active = false;
    await vscode.commands.executeCommand("setContext", CONTEXT_KEY, false);
    this.changed.fire(false);
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
    const client = this.client();
    if (!client) throw new Error("The Phoxite language server isn't running");
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

/** What went wrong, as the server or the editor put it. */
export const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** `2 files` / `1 file`. */
export const count = (n: number, noun: string) => `${n} ${noun}${n === 1 ? "" : "s"}`;

/** The file name of a URI. */
export const baseName = (uri: vscode.Uri) => uri.path.slice(uri.path.lastIndexOf("/") + 1);
