// The Laravel commands answered with the editor's own pickers and input boxes: the
// generators, Artisan commands, "Create translation key…" and the generators'
// templates.

import * as fs from "node:fs/promises";
import * as path from "node:path";
import * as vscode from "vscode";
import { FAILED, registerServerCommand, shown } from "../errors";
import { runSteps, type Step } from "../steps";
import { type Laravel, type Project, baseName, count, message } from "./api";
import { type Answer, type At, type Choice, askChoice, askText, askTicks } from "./input";
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
  schemaMessage,
} from "./model";
import { openNewModel } from "./pages";

const NOT_LARAVEL = "Laravel generators need a Laravel project in this workspace.";

// ---------------------------------------------------------------------------
// Generators.

/** The model list of a generator's dialog; `none` is the entry for no model. */
function modelItems(ctx: GenerationContext, none?: string): Array<Choice<string>> {
  const items: Array<Choice<string>> = ctx.models.map((model) => ({ label: shortName(model), description: model, value: model }));
  if (none) items.unshift({ label: none, value: "" });
  return items;
}

/**
 * Asks what generator `generator` needs as one input of several steps: the name, the
 * module, then the options the server lists for it. Back at a step returns to the one
 * before, with what was answered there. `undefined` when the user gives up.
 */
async function askClass(ctx: GenerationContext, generator: Generator): Promise<NewClassSpec | undefined> {
  const title = generatorTitle(ctx, generator.id);
  const spec = emptyClassSpec(generator.id, ctx.defaultModule ?? "");
  const options = new Set(generator.options);
  const pivot = nameKind(generator.id) === "none";
  if (options.has("model") && pivot && ctx.models.length < 2) {
    void vscode.window.showInformationMessage("A pivot table needs two Eloquent models, and this project has fewer.");
    return undefined;
  }

  const steps: Step[] = [];
  /** A step that keeps its answer with `set`. */
  const step = <T>(ask: (at: At) => Promise<Answer<T>>, set: (value: T) => void) =>
    steps.push(async (at) => {
      const answer = await ask({ ...at, title });
      if (typeof answer === "string") return answer;
      set(answer.value);
      return "next";
    });
  const text = (prompt: string, get: () => string, set: (value: string) => void, validate?: (value: string) => string | null | undefined) =>
    step((at) => askText(at, { prompt, value: get(), validate }), (value) => set(value.trim()));
  const choice = <T>(placeHolder: string, items: () => Array<Choice<T>>, get: () => T, set: (value: T) => void) =>
    step((at) => askChoice(at, { placeHolder, items: items(), current: get() }), set);

  if (!pivot) {
    text(
      namePlaceholder(generator.id),
      () => spec.name,
      (name) => (spec.name = name),
      (value) => (value.trim() ? nameError(generator.id, value) : "Enter a name"),
    );
  }
  if (ctx.modules.length) {
    const targets: Array<Choice<string>> = [
      { label: "Application", description: ctx.namespace, value: "" },
      ...ctx.modules.map((m) => ({ label: m, description: "module", value: m })),
    ];
    choice("Where the class goes", () => targets, () => spec.module, (module) => (spec.module = module));
  }
  if (options.has("model")) {
    choice(
      pivot ? "The first model" : "The model",
      () => modelItems(ctx, pivot ? undefined : "No model"),
      () => spec.model,
      (model) => (spec.model = model),
    );
    if (options.has("related")) {
      choice(
        "The related model",
        () => modelItems(ctx).filter((m) => m.value !== spec.model),
        () => spec.related,
        (related) => (spec.related = related),
      );
    }
  }
  if (options.has("table")) text("Table (guessed from the name when empty)", () => spec.table, (table) => (spec.table = table));
  if (options.has("event")) {
    text("Event class (optional), e.g. OrderShipped", () => spec.event, (event) => (spec.event = event));
  }
  if (options.has("backing")) {
    choice(
      "Backing type",
      () => [
        { label: "Pure enum", value: "" },
        { label: "Backed by string", value: "string" },
        { label: "Backed by int", value: "int" },
      ],
      () => spec.backing,
      (backing) => (spec.backing = backing),
    );
  }
  if (options.has("status")) {
    text(
      "HTTP status, e.g. 403 (500 when empty)",
      () => (spec.status ? String(spec.status) : ""),
      (status) => (spec.status = Number.parseInt(status, 10) || 0),
      statusError,
    );
  }
  // The inputs of an `ide.json` code generation.
  for (const field of generator.fields) {
    if (field.kind === "checkbox") continue;
    if (field.kind === "select") {
      const items: Array<Choice<string>> = field.options.map((o) => ({
        label: o.label,
        description: o.value === field.default ? "default" : undefined,
        value: o.value,
      }));
      if (field.optional) items.unshift({ label: "None", value: "" });
      choice(field.title, () => items, () => spec.inputs[field.id] ?? field.default, (value) => (spec.inputs[field.id] = value));
      continue;
    }
    steps.push(async (at) => {
      // Not trimmed: the text goes into the generated code as typed.
      const answer = await askText(
        { ...at, title },
        {
          prompt: field.optional ? `${field.title} (optional)` : field.title,
          value: spec.inputs[field.id] ?? field.default,
          validate: (typed) => (field.optional || typed.trim() ? null : `Enter ${field.title}`),
        },
      );
      if (typeof answer === "string") return answer;
      spec.inputs[field.id] = answer.value;
      return "next";
    });
  }
  // Everything ticked or not, in one list.
  type Flag = { on(): boolean; set(on: boolean): void };
  const flags: Array<{ label: string; description?: string; value: Flag }> = [];
  if (options.has("create")) {
    flags.push({ label: "Create the table", value: { on: () => spec.create, set: (on) => (spec.create = on) } });
  }
  if (options.has("queued")) flags.push({ label: "Queued", value: { on: () => spec.queued, set: (on) => (spec.queued = on) } });
  if (options.has("api")) {
    flags.push({
      label: "API",
      description: "no create and edit actions",
      value: { on: () => spec.api, set: (on) => (spec.api = on) },
    });
  }
  for (const field of generator.fields.filter((f) => f.kind === "checkbox")) {
    spec.inputs[field.id] = truthy(field.default) ? "true" : "";
    flags.push({
      label: field.title,
      value: { on: () => spec.inputs[field.id] === "true", set: (on) => (spec.inputs[field.id] = on ? "true" : "") },
    });
  }
  if (flags.length) {
    step(
      (at) => askTicks(at, { placeHolder: "Options", items: flags.map((flag) => ({ ...flag, picked: flag.value.on() })) }),
      (ticked: Flag[]) => {
        for (const flag of flags) flag.value.set(ticked.includes(flag.value));
      },
    );
  }
  return (await runSteps(steps)) ? spec : undefined;
}

/** Runs generator `id` in `project`, asking for what it needs. */
async function generate(
  context: vscode.ExtensionContext,
  laravel: Laravel,
  project: Project,
  ctx: GenerationContext,
  id: string,
): Promise<void> {
  if (id === MODEL_GENERATOR) {
    await openNewModel(context, laravel, project.folder);
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
    const created = await project.apply(await project.generateClass(spec));
    if (created[0]) await vscode.window.showTextDocument(created[0]);
    void vscode.window.showInformationMessage(`Created ${created.map(baseName).join(", ") || "nothing"}.`);
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
  }
}

/**
 * "Generate…": the generators of the project; `id` skips the list. The project is the
 * one `clicked` (a file of the Explorer) is in, else the active editor's: it is settled
 * here, so that what is focused while the questions are asked doesn't change it.
 */
async function pickGenerator(context: vscode.ExtensionContext, laravel: Laravel, id?: string, clicked?: vscode.Uri): Promise<void> {
  const project = clicked ? laravel.of(clicked) : laravel.in();
  // A server that is down is not "no Laravel project": the failure is shown as it is.
  const ctx = await shown(project.generationContext());
  if (ctx === FAILED) return;
  if (!ctx) {
    void vscode.window.showInformationMessage(NOT_LARAVEL);
    return;
  }
  if (typeof id === "string" && id) {
    await generate(context, laravel, project, ctx, id);
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
  if (picked?.id) await generate(context, laravel, project, ctx, picked.id);
}

// ---------------------------------------------------------------------------
// Artisan.

/**
 * The folder Artisan runs in: the Laravel project's (`folder`, a URI). A server that
 * doesn't say where the project is leaves the active editor's folder, else the first one.
 */
function projectFolder(folder: string | undefined): vscode.WorkspaceFolder | undefined {
  const known = folder ? vscode.workspace.getWorkspaceFolder(vscode.Uri.parse(folder)) : undefined;
  if (known) return known;
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

async function runArtisan(project: Project, name: string, args: string): Promise<void> {
  const definition: ArtisanTaskDefinition = { type: ARTISAN_TASK, command: name };
  if (args.trim()) definition.arguments = args.trim();
  await vscode.tasks.executeTask(artisanTask(definition, projectFolder(project.folder) ?? vscode.TaskScope.Workspace));
}

/**
 * Artisan commands as tasks: the project's own commands are listed in Run Task, and
 * `artisan` tasks of `tasks.json` (a command saved with its arguments) run.
 */
function artisanTasks(laravel: Laravel): vscode.TaskProvider {
  return {
    async provideTasks() {
      if (!laravel.isActive || !vscode.workspace.isTrusted) return [];
      // The commands of every Laravel project of the workspace, each run in its folder.
      const projects = laravel.folders.length ? laravel.folders.map((folder) => laravel.in(folder)) : [laravel.in()];
      const tasks: vscode.Task[] = [];
      for (const project of projects) {
        const folder = projectFolder(project.folder);
        if (!folder) continue;
        const commands = (await project.artisanCommands().catch(() => null)) ?? [];
        for (const c of commands) {
          if ((c.source !== "class" && c.source !== "closure") || needsArguments(c)) continue;
          const task = artisanTask({ type: ARTISAN_TASK, command: c.name }, folder);
          task.detail = c.description || undefined;
          tasks.push(task);
        }
      }
      return tasks;
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
  // Artisan runs the project's code.
  if (!vscode.workspace.isTrusted) {
    void vscode.window
      .showWarningMessage(
        "Phoxite: Artisan runs code of the project, so it is off until you trust this workspace.",
        "Manage Workspace Trust",
      )
      .then((choice) => choice && vscode.commands.executeCommand("workbench.trust.manage"));
    return;
  }
  // The project is settled here: the command runs where its list was read.
  const project = laravel.in();
  const commands = await shown(project.artisanCommands());
  if (commands === FAILED) return;
  if (!commands) {
    void vscode.window.showInformationMessage("Artisan commands are listed for Laravel projects.");
    return;
  }
  if (typeof name === "string" && name) {
    const command = commands.find((c) => c.name === name);
    if (!command || !needsArguments(command)) {
      await runArtisan(project, name, "");
      return;
    }
    const args = await askArguments(command);
    if (args !== undefined) await runArtisan(project, name, args);
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
  if (args !== undefined) await runArtisan(project, picked.value.name, args);
}

// ---------------------------------------------------------------------------
// "Create translation key…".

/** The action on a translation key: asks which locales get it and their texts, then writes it. */
async function createTranslation(laravel: Laravel, arg: unknown): Promise<void> {
  const request = translationRequestOf(arg);
  if (!request) return;
  const project = laravel.in();
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
    await project.apply(await project.createTranslation(request.key, texts));
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
      guarded((arg?: unknown) =>
        pickGenerator(context, laravel, typeof arg === "string" ? arg : undefined, arg instanceof vscode.Uri ? arg : undefined),
      ),
    ),
    vscode.commands.registerCommand(
      "phoxite.laravel.artisan",
      guarded(() => artisan(laravel)),
    ),
    // The lens on a console command class: its argument is the command's name.
    registerServerCommand("laravel.runArtisan", (name?: unknown) => artisan(laravel, name)),
    // The action on a translation key: its argument is the request's JSON.
    registerServerCommand("laravel.createTranslation", (arg?: unknown) => createTranslation(laravel, arg)),
    vscode.commands.registerCommand(
      "phoxite.laravel.editTemplate",
      guarded(() => editTemplate(laravel)),
    ),
    vscode.commands.registerCommand(
      "phoxite.laravel.refreshSchema",
      guarded(async () => void vscode.window.showInformationMessage(schemaMessage(await laravel.refreshSchema()))),
    ),
  );
}
