// The Laravel commands answered with the editor's own pickers and input boxes: the
// generators, Artisan commands, "Create translation key…" and the generators'
// templates.

import * as fs from "node:fs/promises";
import * as path from "node:path";
import * as vscode from "vscode";
import { type Laravel, baseName, count, message } from "./api";
import {
  type ArtisanCommand,
  type GenerationContext,
  type Generator,
  type NewClassSpec,
  MODEL_GENERATOR,
  argumentsHelp,
  artisanCommandLine,
  emptyClassSpec,
  generatorEntries,
  generatorTitle,
  missingLocales,
  nameError,
  nameKind,
  namePlaceholder,
  needsArguments,
  shortName,
  statusError,
  translationRequestOf,
  truthy,
} from "./model";
import { openNewModel } from "./pages";

const NOT_LARAVEL = "Laravel generators need a Laravel project in this workspace.";

type Pick<T> = vscode.QuickPickItem & { value: T };

/** Asks for one of `items`; `undefined` when the user backs out. */
async function pick<T>(title: string, placeHolder: string, items: Array<Pick<T>>): Promise<T | undefined> {
  const picked = await vscode.window.showQuickPick(items, { title, placeHolder, matchOnDescription: true });
  return picked?.value;
}

// ---------------------------------------------------------------------------
// Generators.

/** The model list of a generator's dialog; `none` is the entry for no model. */
function modelItems(ctx: GenerationContext, none?: string): Array<Pick<string>> {
  const items: Array<Pick<string>> = ctx.models.map((model) => ({ label: shortName(model), description: model, value: model }));
  if (none) items.unshift({ label: none, value: "" });
  return items;
}

/**
 * Asks what generator `generator` needs, one step at a time: the name, the module,
 * then the options the server lists for it. `undefined` when the user backs out.
 */
async function askClass(ctx: GenerationContext, generator: Generator): Promise<NewClassSpec | undefined> {
  const title = generatorTitle(ctx, generator.id);
  const spec = emptyClassSpec(generator.id, ctx.defaultModule ?? "");
  const options = new Set(generator.options);
  const pivot = nameKind(generator.id) === "none";

  if (!pivot) {
    const name = await vscode.window.showInputBox({
      title,
      prompt: namePlaceholder(generator.id),
      validateInput: (value) => (value.trim() ? nameError(generator.id, value) : null),
    });
    if (!name?.trim()) return undefined;
    spec.name = name.trim();
  }
  if (ctx.modules.length) {
    const targets: Array<Pick<string>> = [
      { label: "Application", description: ctx.namespace, value: "" },
      ...ctx.modules.map((m) => ({ label: m, description: "module", value: m })),
    ];
    // The module of the settings first: Enter takes it.
    targets.sort((a, b) => Number(b.value === spec.module) - Number(a.value === spec.module));
    const module = await pick(title, "Where the class goes", targets);
    if (module === undefined) return undefined;
    spec.module = module;
  }
  if (options.has("model")) {
    if (pivot && ctx.models.length < 2) {
      void vscode.window.showInformationMessage("A pivot table needs two Eloquent models, and this project has fewer.");
      return undefined;
    }
    const model = await pick(title, pivot ? "The first model" : "The model", modelItems(ctx, pivot ? undefined : "No model"));
    if (model === undefined) return undefined;
    spec.model = model;
    if (options.has("related")) {
      const related = await pick(title, "The related model", modelItems(ctx).filter((m) => m.value !== model));
      if (related === undefined) return undefined;
      spec.related = related;
    }
  }
  if (options.has("table")) {
    const table = await vscode.window.showInputBox({ title, prompt: "Table (guessed from the name when empty)" });
    if (table === undefined) return undefined;
    spec.table = table.trim();
  }
  if (options.has("event")) {
    const event = await vscode.window.showInputBox({ title, prompt: "Event class (optional), e.g. OrderShipped" });
    if (event === undefined) return undefined;
    spec.event = event.trim();
  }
  if (options.has("backing")) {
    const backing = await pick(title, "Backing type", [
      { label: "Pure enum", value: "" },
      { label: "Backed by string", value: "string" },
      { label: "Backed by int", value: "int" },
    ]);
    if (backing === undefined) return undefined;
    spec.backing = backing;
  }
  if (options.has("status")) {
    const status = await vscode.window.showInputBox({
      title,
      prompt: "HTTP status, e.g. 403 (500 when empty)",
      validateInput: statusError,
    });
    if (status === undefined) return undefined;
    spec.status = Number.parseInt(status.trim(), 10) || 0;
  }
  // The inputs of an `ide.json` code generation.
  for (const field of generator.fields) {
    if (field.kind === "checkbox") continue;
    if (field.kind === "select") {
      const items: Array<Pick<string>> = field.options.map((o) => ({
        label: o.label,
        description: o.value === field.default ? "default" : undefined,
        value: o.value,
      }));
      if (field.optional) items.unshift({ label: "None", value: "" });
      const value = await pick(title, field.title, items);
      if (value === undefined) return undefined;
      spec.inputs[field.id] = value;
      continue;
    }
    const value = await vscode.window.showInputBox({
      title,
      prompt: field.optional ? `${field.title} (optional)` : field.title,
      value: field.default,
      validateInput: (typed) => (field.optional || typed.trim() ? null : `Enter ${field.title}`),
    });
    if (value === undefined) return undefined;
    spec.inputs[field.id] = value;
  }
  // Everything ticked or not, in one list.
  type Flag = { set(on: boolean): void };
  const flags: Array<Pick<Flag>> = [];
  if (options.has("create")) {
    flags.push({ label: "Create the table", picked: spec.create, value: { set: (on) => (spec.create = on) } });
  }
  if (options.has("queued")) flags.push({ label: "Queued", picked: spec.queued, value: { set: (on) => (spec.queued = on) } });
  if (options.has("api")) {
    flags.push({
      label: "API",
      description: "no create and edit actions",
      picked: spec.api,
      value: { set: (on) => (spec.api = on) },
    });
  }
  for (const field of generator.fields.filter((f) => f.kind === "checkbox")) {
    flags.push({
      label: field.title,
      picked: truthy(field.default),
      value: { set: (on) => (spec.inputs[field.id] = on ? "true" : "") },
    });
  }
  if (flags.length) {
    const ticked = await vscode.window.showQuickPick(flags, { title, placeHolder: "Options", canPickMany: true });
    if (!ticked) return undefined;
    for (const flag of flags) flag.value.set(ticked.includes(flag));
  }
  return spec;
}

/** Runs generator `id`, asking for what it needs. */
async function generate(context: vscode.ExtensionContext, laravel: Laravel, ctx: GenerationContext, id: string): Promise<void> {
  if (id === MODEL_GENERATOR) {
    await openNewModel(context, laravel);
    return;
  }
  const generator = ctx.generators.find((g) => g.id === id);
  if (!generator) {
    void vscode.window.showErrorMessage(`Phoxite: this project has no generator '${id}'.`);
    return;
  }
  const spec = await askClass(ctx, generator);
  if (!spec) return;
  try {
    const created = await laravel.apply(await laravel.generateClass(spec));
    if (created[0]) await vscode.window.showTextDocument(created[0]);
    void vscode.window.showInformationMessage(`Created ${created.map(baseName).join(", ") || "nothing"}.`);
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
  }
}

/** "Generate…": the generators of the project; `id` skips the list. */
async function pickGenerator(context: vscode.ExtensionContext, laravel: Laravel, id?: unknown): Promise<void> {
  const ctx = await laravel.generationContext().catch(() => null);
  if (!ctx) {
    void vscode.window.showInformationMessage(NOT_LARAVEL);
    return;
  }
  if (typeof id === "string" && id) {
    await generate(context, laravel, ctx, id);
    return;
  }
  const items: Array<vscode.QuickPickItem & { id?: string }> = generatorEntries(ctx).map((entry) =>
    "heading" in entry
      ? { label: entry.heading, kind: vscode.QuickPickItemKind.Separator }
      : { label: entry.title, description: entry.description, id: entry.id },
  );
  const picked = await vscode.window.showQuickPick(items, {
    title: "Laravel: Generate",
    placeHolder: "What to create",
    matchOnDescription: true,
  });
  if (picked?.id) await generate(context, laravel, ctx, picked.id);
}

// ---------------------------------------------------------------------------
// Artisan.

/** The folder Artisan runs in: the active editor's workspace folder, else the first one. */
function projectFolder(): vscode.WorkspaceFolder | undefined {
  const uri = vscode.window.activeTextEditor?.document.uri;
  return (uri && vscode.workspace.getWorkspaceFolder(uri)) ?? vscode.workspace.workspaceFolders?.[0];
}

/** The type of the Artisan tasks (`taskDefinitions` in `package.json`). */
const ARTISAN_TASK = "artisan";

/** A task of `tasks.json`: `{ "type": "artisan", "command": "migrate", "arguments": "--seed" }`. */
interface ArtisanTaskDefinition extends vscode.TaskDefinition {
  command: string;
  arguments?: string;
}

/** The task running `artisan <command> <arguments>` in `scope`, in the terminal panel. */
function artisanTask(definition: ArtisanTaskDefinition, scope: vscode.WorkspaceFolder | vscode.TaskScope): vscode.Task {
  const folder = typeof scope === "object" ? scope : undefined;
  const artisan = vscode.workspace.getConfiguration("phoxite", folder).get<string>("laravel.artisanCommand", "php artisan");
  const line = artisanCommandLine(artisan, definition.command, definition.arguments ?? "");
  const task = new vscode.Task(
    definition,
    scope,
    [definition.command, definition.arguments?.trim()].filter(Boolean).join(" "),
    ARTISAN_TASK,
    new vscode.ShellExecution(line, { cwd: folder?.uri.fsPath }),
  );
  task.presentationOptions = { reveal: vscode.TaskRevealKind.Always, panel: vscode.TaskPanelKind.Dedicated, clear: true };
  return task;
}

async function runArtisan(name: string, args: string): Promise<void> {
  const definition: ArtisanTaskDefinition = { type: ARTISAN_TASK, command: name };
  if (args.trim()) definition.arguments = args.trim();
  await vscode.tasks.executeTask(artisanTask(definition, projectFolder() ?? vscode.TaskScope.Workspace));
}

/**
 * Artisan commands as tasks: the project's own commands are listed in Run Task, and
 * `artisan` tasks of `tasks.json` (a command saved with its arguments) run.
 */
function artisanTasks(laravel: Laravel): vscode.TaskProvider {
  return {
    async provideTasks() {
      const folder = projectFolder();
      if (!laravel.isActive || !folder) return [];
      const commands = (await laravel.artisanCommands().catch(() => null)) ?? [];
      return commands
        .filter((c) => (c.source === "class" || c.source === "closure") && !needsArguments(c))
        .map((c) => {
          const task = artisanTask({ type: ARTISAN_TASK, command: c.name }, folder);
          task.detail = c.description || undefined;
          return task;
        });
    },
    resolveTask(task) {
      const definition = task.definition as ArtisanTaskDefinition;
      if (typeof definition.command !== "string" || !definition.command.trim()) return undefined;
      const scope = task.scope === undefined || task.scope === vscode.TaskScope.Global ? vscode.TaskScope.Workspace : task.scope;
      return artisanTask(definition, scope);
    },
  };
}

/** Asks for the arguments of `command`; `undefined` when the user backs out. */
function askArguments(command: ArtisanCommand): Thenable<string | undefined> {
  return vscode.window.showInputBox({
    title: `php artisan ${command.synopsis}`,
    prompt: argumentsHelp(command) || "Arguments and options",
    placeHolder: "Arguments and options",
    validateInput: (value) => (needsArguments(command) && !value.trim() ? "This command has required arguments" : null),
  });
}

const SOURCES: Record<ArtisanCommand["source"], string> = {
  class: "this project",
  closure: "routes/console.php",
  package: "a package",
  framework: "Laravel",
};

/**
 * "Run Artisan Command…": lists the project's commands, asks for arguments, runs.
 * With `name` (the lens on a console command class) the command runs at once unless
 * it has required arguments.
 */
async function artisan(laravel: Laravel, name?: unknown): Promise<void> {
  const commands = await laravel.artisanCommands().catch(() => null);
  if (!commands) {
    void vscode.window.showInformationMessage("Artisan commands are listed for Laravel projects.");
    return;
  }
  if (typeof name === "string" && name) {
    const command = commands.find((c) => c.name === name);
    if (!command || !needsArguments(command)) {
      await runArtisan(name, "");
      return;
    }
    const args = await askArguments(command);
    if (args !== undefined) await runArtisan(name, args);
    return;
  }
  // The project's own commands first.
  const own = commands.filter((c) => c.source === "class" || c.source === "closure");
  const others = commands.filter((c) => c.source !== "class" && c.source !== "closure");
  const item = (command: ArtisanCommand) => ({
    label: command.name,
    description: command.description,
    detail: command.source === "framework" ? undefined : `From ${SOURCES[command.source]}`,
    value: command,
  });
  const items: Array<vscode.QuickPickItem & { value?: ArtisanCommand }> = [];
  if (own.length) items.push({ label: "This project", kind: vscode.QuickPickItemKind.Separator }, ...own.map(item));
  if (others.length) items.push({ label: "Laravel and packages", kind: vscode.QuickPickItemKind.Separator }, ...others.map(item));
  const picked = await vscode.window.showQuickPick(items, {
    title: "Run Artisan Command",
    placeHolder: "Search commands by name or description",
    matchOnDescription: true,
  });
  if (!picked?.value) return;
  const args = await askArguments(picked.value);
  if (args !== undefined) await runArtisan(picked.value.name, args);
}

// ---------------------------------------------------------------------------
// "Create translation key…".

/** The action on a translation key: asks which locales get it and their texts, then writes it. */
async function createTranslation(laravel: Laravel, arg: unknown): Promise<void> {
  const request = translationRequestOf(arg);
  if (!request) return;
  const missing = missingLocales(request);
  if (!missing.length) {
    void vscode.window.showInformationMessage(`Every locale has '${request.key}' already.`);
    return;
  }
  const title = `Create translation key '${request.key}'`;
  let chosen = missing;
  if (missing.length > 1) {
    const picked = await vscode.window.showQuickPick(
      missing.map((l) => ({ label: l.locale, description: l.text, picked: true, value: l })),
      { title, placeHolder: "The locales to add it to", canPickMany: true },
    );
    if (!picked) return;
    chosen = picked.map((p) => p.value);
  }
  if (!chosen.length) return;
  const texts: Array<{ locale: string; text: string }> = [];
  for (const locale of chosen) {
    const text = await vscode.window.showInputBox({
      title,
      prompt: `The text in ${locale.locale}`,
      value: locale.text,
      validateInput: (value) => (value.trim() ? null : "Enter the text"),
    });
    if (text === undefined) return;
    texts.push({ locale: locale.locale, text });
  }
  try {
    await laravel.apply(await laravel.createTranslation(request.key, texts));
    vscode.window.setStatusBarMessage(`Created '${request.key}' in ${count(texts.length, "locale")}`, 4000);
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
  }
}

// ---------------------------------------------------------------------------
// The generators' templates.

/** "Edit Generator Template…": opens the project's copy of a template, made from the default when there is none. */
async function editTemplate(laravel: Laravel): Promise<void> {
  let templates;
  try {
    templates = await laravel.templates();
  } catch (e) {
    void vscode.window.showInformationMessage(message(e));
    return;
  }
  const picked = await vscode.window.showQuickPick(
    templates.map((t) => ({ label: t.name, description: t.edited ? "edited" : undefined, value: t })),
    { title: "Edit Generator Template", placeHolder: "The generators render these; an edited one replaces the default" },
  );
  if (!picked) return;
  const template = picked.value;
  if (!template.edited) {
    await fs.mkdir(path.dirname(template.path), { recursive: true });
    // `wx`: never over a file that appeared meanwhile.
    await fs.writeFile(template.path, template.body, { flag: "wx" }).catch((e: NodeJS.ErrnoException) => {
      if (e.code !== "EEXIST") throw e;
    });
  }
  await vscode.window.showTextDocument(vscode.Uri.file(template.path));
}

export function registerCommands(context: vscode.ExtensionContext, laravel: Laravel): void {
  const guarded =
    <A extends unknown[]>(run: (...args: A) => Promise<void>) =>
    (...args: A) =>
      run(...args).catch((e) => void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`));
  context.subscriptions.push(
    vscode.tasks.registerTaskProvider(ARTISAN_TASK, artisanTasks(laravel)),
    vscode.commands.registerCommand(
      "phoxite.laravel.generate",
      // From the explorer's menu the argument is the clicked file, not a generator.
      guarded((id?: unknown) => pickGenerator(context, laravel, typeof id === "string" ? id : undefined)),
    ),
    vscode.commands.registerCommand(
      "phoxite.laravel.artisan",
      guarded(() => artisan(laravel)),
    ),
    // The lens on a console command class: its argument is the command's name.
    vscode.commands.registerCommand(
      "laravel.runArtisan",
      guarded((name?: unknown) => artisan(laravel, name)),
    ),
    // The action on a translation key: its argument is the request's JSON.
    vscode.commands.registerCommand(
      "laravel.createTranslation",
      guarded((arg?: unknown) => createTranslation(laravel, arg)),
    ),
    vscode.commands.registerCommand(
      "phoxite.laravel.editTemplate",
      guarded(() => editTemplate(laravel)),
    ),
  );
}
