// The refactorings that ask something first. The server offers three of them as
// code actions running a command of this file with the action's data (Change
// Signature, Extract Method, the Generate member choosers); Pull Members Up and
// Push Members Down are commands of their own. Each asks, sends the answer to the
// server and applies the edit it computes.

import * as vscode from "vscode";
import type { LanguageClient, WorkspaceEdit as ProtocolEdit } from "vscode-languageclient/node";
import { changeSignaturePage } from "./html";
import {
  type ChangeSignatureData,
  type ExtractMethodData,
  type HierarchyInfo,
  type HierarchyKind,
  type HierarchyProblem,
  type MemberChooserData,
  type Signature,
  VISIBILITIES,
  abstractPrompt,
  extractChoice,
  extractNameError,
  extractOptions,
  hierarchyChoices,
  signatureError,
  splitProblems,
  tidySignature,
} from "./model";

/** The id of the Change Signature form's pages. */
const CHANGE_SIGNATURE = "phoxite.changeSignature";

/** The code action kind the server gives "Change signature". */
const CHANGE_SIGNATURE_KIND = "refactor.rewrite.changeSignature";

const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

export class Refactorings {
  constructor(private readonly client: () => LanguageClient | undefined) {}

  private running(): LanguageClient {
    const client = this.client();
    if (!client?.isRunning()) throw new Error("The Phoxite language server isn't running");
    return client;
  }

  request<T>(method: string, params: Record<string, unknown>): Promise<T> {
    return this.running().sendRequest<T>(`phoxite/refactor/${method}`, params);
  }

  /** Applies an edit of the server, as one undoable refactoring. */
  async apply(edit: ProtocolEdit): Promise<void> {
    const converted = await this.running().protocol2CodeConverter.asWorkspaceEdit(edit);
    if (!converted || !(await vscode.workspace.applyEdit(converted, { isRefactoring: true }))) {
      throw new Error("The edit couldn't be applied");
    }
  }

  /** Sends the answered `data` of a code action and applies the edit. */
  async resolve(method: string, uri: string, data: unknown): Promise<void> {
    const { edit } = await this.request<{ edit: ProtocolEdit }>(method, { uri, data });
    await this.apply(edit);
  }
}

function failed(e: unknown): void {
  void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
}

// ---------------------------------------------------------------------------
// The Generate member choosers.

async function generateMembers(refactorings: Refactorings, uri: string, data: MemberChooserData): Promise<void> {
  if (!uri || !data?.members) return;
  const picked = await vscode.window.showQuickPick(
    data.members.map((m) => ({ label: m.label, description: m.detail || undefined, picked: m.selected, id: m.id })),
    { title: data.title.replace(/…$/, ""), placeHolder: "Members", canPickMany: true, matchOnDescription: true },
  );
  if (!picked?.length) return;
  try {
    await refactorings.resolve("generate", uri, { ...data, chosen: picked.map((p) => p.id) });
  } catch (e) {
    failed(e);
  }
}

// ---------------------------------------------------------------------------
// Extract Method.

async function extractMethod(refactorings: Refactorings, uri: string, data: ExtractMethodData): Promise<void> {
  if (!uri || !data?.defaults) return;
  const what = data.function ? "function" : "method";
  const title = `Extract ${data.function ? "Function" : `Method in ${data.className}`}`;
  const name = await vscode.window.showInputBox({
    title,
    prompt: `The new ${what}'s name`,
    value: data.defaults.name,
    validateInput: extractNameError,
  });
  if (name === undefined) return;
  let visibility = data.defaults.visibility;
  if (!data.function) {
    const order = [visibility, ...VISIBILITIES.filter((v) => v !== visibility)];
    const picked = await vscode.window.showQuickPick(order, { title, placeHolder: "Visibility" });
    if (!picked) return;
    visibility = picked;
  }
  const options = extractOptions(data);
  let ticked = options.filter((o) => o.picked).map((o) => o.id);
  if (options.length) {
    const picked = await vscode.window.showQuickPick(
      options.map((o) => ({ label: o.label, description: o.description, picked: o.picked, id: o.id })),
      { title, placeHolder: "Options", canPickMany: true },
    );
    if (!picked) return;
    ticked = picked.map((p) => p.id);
  }
  try {
    await refactorings.resolve("extractMethod", uri, { ...data, change: extractChoice(data, name, visibility, ticked) });
  } catch (e) {
    failed(e);
  }
}

// ---------------------------------------------------------------------------
// Change Signature.

function changeSignature(
  context: vscode.ExtensionContext,
  refactorings: Refactorings,
  uri: string,
  data: ChangeSignatureData,
): void {
  if (!uri || !data?.signature) return;
  const media = vscode.Uri.joinPath(context.extensionUri, "media");
  const view = vscode.window.createWebviewPanel(CHANGE_SIGNATURE, "Change Signature", vscode.ViewColumn.Beside, {
    enableScripts: true,
    localResourceRoots: [media],
  });
  const file = (name: string) => view.webview.asWebviewUri(vscode.Uri.joinPath(media, name)).toString();
  view.webview.html = changeSignaturePage(data.signature, {
    cspSource: view.webview.cspSource,
    style: file("laravel.css"),
    script: file("changeSignature.js"),
  });
  const apply = async (wanted: Signature): Promise<void> => {
    const change = tidySignature(wanted);
    const invalid = signatureError(data.signature, change);
    if (invalid) {
      await view.webview.postMessage({ type: "error", message: invalid });
      return;
    }
    try {
      await refactorings.resolve("changeSignature", uri, { ...data, change });
    } catch (e) {
      await view.webview.postMessage({ type: "error", message: message(e) });
      return;
    }
    view.dispose();
  };
  view.webview.onDidReceiveMessage((m: { type?: string; signature?: Signature }) => {
    if (m.type === "cancel") view.dispose();
    else if (m.type === "apply" && m.signature) void apply(m.signature);
  });
}

/** "Change Signature…" from the palette: the action the server offers at the caret. */
async function changeSignatureAtCaret(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const actions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
    "vscode.executeCodeActionProvider",
    editor.document.uri,
    editor.selection,
    CHANGE_SIGNATURE_KIND,
  );
  const command = actions?.find((a) => a.command?.command === CHANGE_SIGNATURE)?.command;
  if (!command) {
    void vscode.window.showInformationMessage("Put the caret on a function or method, or on a call of one.");
    return;
  }
  await vscode.commands.executeCommand(command.command, ...(command.arguments ?? []));
}

// ---------------------------------------------------------------------------
// Pull Members Up / Push Members Down.

async function moveMembers(refactorings: Refactorings, kind: HierarchyKind): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor || editor.document.languageId !== "php") return;
  const title = kind === "pullUp" ? "Pull Members Up" : "Push Members Down";
  const uri = editor.document.uri.toString();
  const at = editor.selection.active;
  const position = { line: at.line, character: at.character };
  try {
    const info = await refactorings.request<HierarchyInfo>("hierarchyMembers", { uri, position, kind });
    let target = info.targets[0];
    if (kind === "pullUp" && info.targets.length > 1) {
      const picked = await vscode.window.showQuickPick(
        info.targets.map((t) => ({
          label: t.name,
          description: t.qualifiedName,
          detail: t.interface ? "interface" : undefined,
          target: t,
        })),
        { title, placeHolder: `Where the members of ${info.className} go` },
      );
      if (!picked) return;
      target = picked.target;
    }
    const where = kind === "pullUp" ? `to ${target?.name ?? "the parent"}` : "into the subclasses";
    const members = await vscode.window.showQuickPick(
      info.members.map((m) => ({ label: m.label, description: m.kind, picked: m.selected, member: m })),
      { title, placeHolder: `Members of ${info.className} to move ${where}`, canPickMany: true },
    );
    if (!members?.length) return;
    const moved = members.map((m) => m.member.id);
    let abstracted: string[] = [];
    const prompt = abstractPrompt(kind, target);
    const abstractable = members.filter((m) => m.member.abstractable && m.member.kind === "method");
    if (prompt && abstractable.length) {
      const picked = await vscode.window.showQuickPick(
        abstractable.map((m) => ({ label: m.label, member: m.member })),
        { title, placeHolder: prompt, canPickMany: true },
      );
      if (!picked) return;
      abstracted = picked.map((p) => p.member.id);
    }
    const result = await refactorings.request<{ problems: HierarchyProblem[]; edit: ProtocolEdit | null }>("hierarchy", {
      uri,
      position,
      kind,
      target: kind === "pullUp" ? target?.id : undefined,
      choices: hierarchyChoices(info, moved, abstracted),
    });
    const { fatal, confirm } = splitProblems(result.problems);
    if (fatal.length || !result.edit) {
      void vscode.window.showErrorMessage(`${title}: ${fatal.join(" ") || "nothing can be moved"}`);
      return;
    }
    if (confirm.length) {
      const go = await vscode.window.showWarningMessage(title, { modal: true, detail: confirm.join("\n") }, "Refactor Anyway");
      if (!go) return;
    }
    await refactorings.apply(result.edit);
  } catch (e) {
    failed(e);
  }
}

export function registerRefactorings(context: vscode.ExtensionContext, refactorings: Refactorings): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("phoxite.generateMembers", (uri: string, data: MemberChooserData) =>
      generateMembers(refactorings, uri, data),
    ),
    vscode.commands.registerCommand("phoxite.extractMethod", (uri: string, data: ExtractMethodData) =>
      extractMethod(refactorings, uri, data),
    ),
    vscode.commands.registerCommand(CHANGE_SIGNATURE, (uri: string, data: ChangeSignatureData) =>
      changeSignature(context, refactorings, uri, data),
    ),
    vscode.commands.registerCommand("phoxite.refactor.changeSignature", changeSignatureAtCaret),
    vscode.commands.registerCommand("phoxite.refactor.pullMembersUp", () => moveMembers(refactorings, "pullUp")),
    vscode.commands.registerCommand("phoxite.refactor.pushMembersDown", () => moveMembers(refactorings, "pushDown")),
  );
}
