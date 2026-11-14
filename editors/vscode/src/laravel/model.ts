// What the server's `phoxite/laravel/*` requests carry, and the rules of the
// dialogs built on them. No `vscode` here: the tests run this in plain Node.

export interface Pos {
  line: number;
  col: number;
}

export interface Range {
  start: Pos;
  end: Pos;
}

/** A place in a file, as the engine names it. */
export interface Place {
  path: string;
  range: Range;
}

// ---------------------------------------------------------------------------
// The Eloquent model view.

export interface ModelColumn {
  name: string;
  /** The PHP type reading the column gives (before casts). */
  ty: string;
  nullable: boolean;
  cast: string | null;
  source: "migration" | "database";
  database: string | null;
  fillable: boolean;
  hidden: boolean;
  location: Place | null;
}

export interface ModelRelation {
  name: string;
  /** `HasMany`, `BelongsTo`… */
  kind: string;
  related: string | null;
  many: boolean;
  location: Place | null;
}

/** A local scope or an accessor. */
export interface ModelMember {
  name: string;
  method: string;
  ty: string | null;
  location: Place | null;
}

export interface RelatedClass {
  kind: "factory" | "resource" | "policy" | "observer" | "migration";
  name: string;
  location: Place;
}

export interface ModelView {
  class: string;
  name: string;
  location: Place | null;
  table: string;
  primaryKey: string;
  keyType: string;
  incrementing: boolean;
  /** The columns are known for certain (a migration creates the table). */
  schemaCertain: boolean;
  columns: ModelColumn[];
  relations: ModelRelation[];
  scopes: ModelMember[];
  accessors: ModelMember[];
  related: RelatedClass[];
}

/** The column types offered (Laravel's `Blueprint` methods). */
export const COLUMN_TYPES = [
  "string",
  "text",
  "integer",
  "bigInteger",
  "unsignedBigInteger",
  "foreignId",
  "boolean",
  "decimal",
  "float",
  "date",
  "dateTime",
  "timestamp",
  "json",
  "uuid",
] as const;

/** A column the model view's Add Columns form adds. */
export interface NewColumn {
  name: string;
  ty: string;
  nullable: boolean;
}

const IDENTIFIER = /^[A-Za-z_][A-Za-z0-9_]*$/;
const CLASS_NAME = /^[A-Za-z_][A-Za-z0-9_]*([\\/][A-Za-z_][A-Za-z0-9_]*)*$/;

/** The columns to add: the rows with a name. */
export function chosenColumns(rows: readonly NewColumn[]): NewColumn[] {
  return rows
    .filter((r) => typeof r?.name === "string" && r.name.trim())
    .map((r) => ({ name: r.name.trim(), ty: String(r.ty || "string"), nullable: !!r.nullable }));
}

/** Why the columns can't be added yet, or null (the server checks the rest). */
export function columnsError(rows: readonly NewColumn[], existing: readonly string[]): string | null {
  const columns = chosenColumns(rows);
  if (!columns.length) return "Enter at least one column name";
  const seen = new Set(existing.map((name) => name.toLowerCase()));
  for (const c of columns) {
    if (!IDENTIFIER.test(c.name)) return `'${c.name}' is not a valid column name`;
    if (seen.has(c.name.toLowerCase())) return `The table has a column '${c.name}' already`;
    seen.add(c.name.toLowerCase());
  }
  return null;
}

const pad = (n: number, width = 2) => String(n).padStart(width, "0");

/** A migration's file-name prefix (`2024_05_01_120000`), as `php artisan make:migration` writes it. */
export function migrationTimestamp(date: Date): string {
  return `${pad(date.getFullYear(), 4)}_${pad(date.getMonth() + 1)}_${pad(date.getDate())}_${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
}

/** `App\Models\Post` → `Post`. */
export const shortName = (fqn: string) => fqn.slice(fqn.lastIndexOf("\\") + 1);

// ---------------------------------------------------------------------------
// Generators.

export interface FieldOption {
  value: string;
  label: string;
}

/** An input field of an `ide.json` code generation. */
export interface GeneratorField {
  id: string;
  title: string;
  kind: "text" | "checkbox" | "select" | string;
  variable: string;
  optional: boolean;
  /** The value when left empty (`true` for a ticked checkbox). */
  default: string;
  options: FieldOption[];
}

export interface Generator {
  /** `model`, `controller`, `ide-json:<id>`. */
  id: string;
  title: string;
  description: string;
  templates: string[];
  /** What its dialog asks for: `model`, `related`, `table`, `create`, `event`, `queued`, `api`, `backing`, `status`. */
  options: string[];
  fields: GeneratorField[];
}

export interface GenerationContext {
  /** The application's namespace, with a trailing `\`. */
  namespace: string;
  modules: string[];
  defaultModule: string | null;
  /** The project's Eloquent models (FQNs). */
  models: string[];
  generators: Generator[];
}

export const GENERATOR_GROUPS = [
  { id: "http", label: "HTTP Layer" },
  { id: "database", label: "Database" },
  { id: "events", label: "Events and Jobs" },
  { id: "auth", label: "Authorization and Validation" },
  { id: "views", label: "Views" },
  { id: "other", label: "Other" },
  { id: "tests", label: "Tests" },
] as const;

export type GeneratorGroup = (typeof GENERATOR_GROUPS)[number]["id"];

/** The class generators the extension has titles for (the server's ids), in menu order. */
export const CLASS_GENERATORS: ReadonlyArray<{ id: string; title: string; group: GeneratorGroup }> = [
  { id: "controller", title: "Controller", group: "http" },
  { id: "resource-controller", title: "Resource Controller", group: "http" },
  { id: "middleware", title: "Middleware", group: "http" },
  { id: "request", title: "Form Request", group: "http" },
  { id: "resource", title: "JSON Resource", group: "http" },
  { id: "resource-collection", title: "Resource Collection", group: "http" },
  { id: "factory", title: "Factory", group: "database" },
  { id: "seeder", title: "Seeder", group: "database" },
  { id: "migration", title: "Migration", group: "database" },
  { id: "pivot-migration", title: "Pivot Table Migration", group: "database" },
  { id: "event", title: "Event", group: "events" },
  { id: "listener", title: "Listener", group: "events" },
  { id: "job", title: "Job", group: "events" },
  { id: "mail", title: "Mailable", group: "events" },
  { id: "notification", title: "Notification", group: "events" },
  { id: "observer", title: "Observer", group: "events" },
  { id: "channel", title: "Broadcasting Channel", group: "events" },
  { id: "policy", title: "Policy", group: "auth" },
  { id: "rule", title: "Validation Rule", group: "auth" },
  { id: "component", title: "Blade Component", group: "views" },
  { id: "anonymous-component", title: "Anonymous Blade Component", group: "views" },
  { id: "view", title: "Blade View", group: "views" },
  { id: "view-composer", title: "View Composer", group: "views" },
  { id: "livewire", title: "Livewire Component", group: "views" },
  { id: "exception", title: "Exception", group: "other" },
  { id: "http-exception", title: "HTTP Exception", group: "other" },
  { id: "provider", title: "Service Provider", group: "other" },
  { id: "command", title: "Console Command", group: "other" },
  { id: "enum", title: "Enum", group: "other" },
  { id: "feature-test", title: "Feature Test", group: "tests" },
  { id: "unit-test", title: "Unit Test", group: "tests" },
];

export const MODEL_GENERATOR = "model";
const IDE_JSON = "ide-json:";

/** An entry of the generator list: a generator, or the heading of the ones after it. */
export type GeneratorEntry = { heading: string } | { id: string; title: string; description: string };

/**
 * The generators the server offers, as the list shows them: New Model, the class
 * generators by group, the project's `ide.json` code generations, and anything the
 * server has that this extension has no title for.
 */
export function generatorEntries(ctx: GenerationContext): GeneratorEntry[] {
  const offered = new Map(ctx.generators.map((g) => [g.id, g]));
  const out: GeneratorEntry[] = [];
  const shown = new Set<string>();
  const model = offered.get(MODEL_GENERATOR);
  if (model) {
    out.push({ id: model.id, title: "Eloquent Model", description: "Fields, relations, migration, factory, controller…" });
    shown.add(model.id);
  }
  for (const group of GENERATOR_GROUPS) {
    const members = CLASS_GENERATORS.filter((g) => g.group === group.id && offered.has(g.id));
    if (!members.length) continue;
    out.push({ heading: group.label });
    for (const g of members) {
      out.push({ id: g.id, title: g.title, description: offered.get(g.id)!.description });
      shown.add(g.id);
    }
  }
  const rest = ctx.generators.filter((g) => !shown.has(g.id));
  const custom = rest.filter((g) => g.id.startsWith(IDE_JSON));
  const unknown = rest.filter((g) => !g.id.startsWith(IDE_JSON));
  if (custom.length) out.push({ heading: "ide.json" });
  for (const g of custom) out.push({ id: g.id, title: g.title, description: g.description });
  if (unknown.length) out.push({ heading: "More" });
  for (const g of unknown) out.push({ id: g.id, title: g.title, description: g.description });
  return out;
}

/** The title of generator `id`'s dialog. */
export function generatorTitle(ctx: GenerationContext, id: string): string {
  const known = CLASS_GENERATORS.find((g) => g.id === id)?.title ?? ctx.generators.find((g) => g.id === id)?.title ?? id;
  return `New ${known}`;
}

/** What the server's class generators take (`NewClass`). */
export interface NewClassSpec {
  generator: string;
  name: string;
  /** `null` = the module of the settings, "" = the application. */
  module: string | null;
  model: string;
  related: string;
  table: string;
  create: boolean;
  event: string;
  queued: boolean;
  api: boolean;
  backing: string;
  status: number;
  inputs: Record<string, string>;
}

export function emptyClassSpec(generator: string, module: string | null = null): NewClassSpec {
  return {
    generator,
    name: "",
    module,
    model: "",
    related: "",
    table: "",
    create: false,
    event: "",
    queued: true,
    api: false,
    backing: "",
    status: 0,
    inputs: {},
  };
}

const VIEW_NAME = /^[A-Za-z0-9_-]+([./][A-Za-z0-9_-]+)*$/;
const MIGRATION_NAME = /^[A-Za-z0-9_ -]+$/;
/** `posts.create-post` or `Posts/CreatePost`, as `make:livewire` takes it. */
const LIVEWIRE_NAME = /^[A-Za-z_][A-Za-z0-9_-]*([./\\][A-Za-z_][A-Za-z0-9_-]*)*$/;

/** What the name of generator `id` is. */
export function nameKind(id: string): "class" | "view" | "migration" | "livewire" | "none" {
  if (id === "view" || id === "anonymous-component") return "view";
  if (id === "livewire") return "livewire";
  if (id === "migration") return "migration";
  if (id === "pivot-migration") return "none";
  return "class";
}

/** What to type as the name of generator `id`. */
export function namePlaceholder(id: string): string {
  switch (nameKind(id)) {
    case "view":
      return id === "view" ? "View name, e.g. posts.index" : "Component name, e.g. forms.input";
    case "migration":
      return "Migration name, e.g. add_votes_to_users_table";
    case "livewire":
      return "Component name, e.g. posts.create-post";
    default:
      return "Class name, e.g. Post or Admin/Post";
  }
}

/** Why `name` can't be the name of generator `id`, or null (the server checks the rest). */
export function nameError(id: string, name: string): string | null {
  const typed = name.trim();
  const kind = nameKind(id);
  if (kind === "none") return null;
  if (!typed) return "Enter a name";
  if (kind === "class" && !CLASS_NAME.test(typed)) return `'${typed}' is not a valid class name`;
  if (kind === "view" && !VIEW_NAME.test(typed.replace(/\.blade\.php$/, ""))) return `'${typed}' is not a valid view name`;
  if (kind === "migration" && !MIGRATION_NAME.test(typed)) return `'${typed}' is not a valid migration name`;
  if (kind === "livewire" && !LIVEWIRE_NAME.test(typed)) return `'${typed}' is not a valid component name`;
  return null;
}

/** Why `status` can't be an HTTP exception's status, or null. Empty is 500. */
export function statusError(status: string): string | null {
  return status.trim() && !/^[1-5]\d{2}$/.test(status.trim()) ? "The status is a three-digit HTTP status" : null;
}

/** A checkbox field's value counts as ticked. */
export const truthy = (value: string) => !["", "false", "0"].includes(value.trim());

export const RELATION_KINDS = ["belongsTo", "hasOne", "hasMany", "belongsToMany", "morphMany", "morphTo"] as const;
export const CONTROLLER_KINDS = [
  { value: "none", label: "No controller" },
  { value: "plain", label: "Plain controller" },
  { value: "resource", label: "Resource controller" },
  { value: "api", label: "API controller" },
] as const;

export interface NewModelField {
  name: string;
  /** The `Blueprint` method making the column. */
  ty: string;
  nullable: boolean;
  unique: boolean;
  index: boolean;
  /** The default value as PHP; empty = none. */
  default: string;
}

export interface NewModelRelation {
  kind: string;
  /** The related model (FQN or short name). */
  model: string;
  /** The method name; empty = from the related model. */
  name: string;
}

/** The New Model form, as its page posts it. */
export interface NewModelForm {
  name: string;
  /** "" = the application. */
  module: string;
  fields: NewModelField[];
  relations: NewModelRelation[];
  /** Comma-separated columns per index. */
  indexes: Array<{ columns: string; unique: boolean }>;
  timestamps: boolean;
  softDeletes: boolean;
  migration: boolean;
  factory: boolean;
  seeder: boolean;
  controller: string;
  formRequests: boolean;
  resource: boolean;
  policy: boolean;
}

/** What the server's New Model generator takes (`NewModel`). */
export type NewModelSpec = Omit<NewModelForm, "indexes"> & { indexes: Array<{ columns: string[]; unique: boolean }> };

export function emptyModelForm(module = ""): NewModelForm {
  return {
    name: "",
    module,
    fields: [{ name: "", ty: "string", nullable: false, unique: false, index: false, default: "" }],
    relations: [],
    indexes: [],
    timestamps: true,
    softDeletes: false,
    migration: true,
    factory: true,
    seeder: false,
    controller: "none",
    formRequests: false,
    resource: false,
    policy: false,
  };
}

const text = (value: unknown) => (typeof value === "string" ? value.trim() : "");

/** The request of the form: the rows with a name, the indexes' columns split. */
export function newModelSpec(form: NewModelForm): NewModelSpec {
  return {
    name: text(form.name),
    module: text(form.module),
    fields: (form.fields ?? [])
      .filter((f) => text(f.name))
      .map((f) => ({
        name: text(f.name),
        ty: text(f.ty) || "string",
        nullable: !!f.nullable,
        unique: !!f.unique,
        index: !!f.index,
        default: text(f.default),
      })),
    relations: (form.relations ?? [])
      .filter((r) => text(r.model) || r.kind === "morphTo")
      .map((r) => ({ kind: text(r.kind), model: r.kind === "morphTo" ? "" : text(r.model), name: text(r.name) })),
    indexes: (form.indexes ?? [])
      .map((i) => ({
        columns: text(i.columns)
          .split(",")
          .map((c) => c.trim())
          .filter(Boolean),
        unique: !!i.unique,
      }))
      .filter((i) => i.columns.length),
    timestamps: !!form.timestamps,
    softDeletes: !!form.softDeletes,
    migration: !!form.migration,
    factory: !!form.factory,
    seeder: !!form.seeder,
    controller: CONTROLLER_KINDS.some((k) => k.value === form.controller) ? form.controller : "none",
    formRequests: !!form.formRequests,
    resource: !!form.resource,
    policy: !!form.policy,
  };
}

/** Why the form can't be submitted yet, or null (the server checks the rest). */
export function newModelError(form: NewModelForm): string | null {
  const spec = newModelSpec(form);
  if (!spec.name) return "Enter the model's class name";
  if (!CLASS_NAME.test(spec.name)) return `'${spec.name}' is not a valid class name`;
  const seen = new Set<string>();
  for (const f of spec.fields) {
    if (!IDENTIFIER.test(f.name)) return `'${f.name}' is not a valid column name`;
    if (seen.has(f.name.toLowerCase())) return `The column '${f.name}' is there twice`;
    seen.add(f.name.toLowerCase());
  }
  for (const r of spec.relations) {
    if (!(RELATION_KINDS as readonly string[]).includes(r.kind)) return `'${r.kind}' is not a relation`;
    if (r.kind === "morphTo" && !r.name) return "Name the morphTo relation (commentable)";
    if (r.name && !IDENTIFIER.test(r.name)) return `'${r.name}' is not a valid relation name`;
  }
  for (const i of spec.indexes) {
    const bad = i.columns.find((c) => !IDENTIFIER.test(c));
    if (bad) return `'${bad}' is not a valid column name`;
  }
  return null;
}

// ---------------------------------------------------------------------------
// Artisan.

export interface ArtisanArgument {
  name: string;
  description: string;
  required: boolean;
  array: boolean;
  default: string | null;
}

export interface ArtisanOption {
  /** Without `--`. */
  name: string;
  shortcut: string | null;
  description: string;
  /** Takes a value (`--queue=`), as opposed to a flag. */
  acceptsValue: boolean;
  array: boolean;
  default: string | null;
}

export interface ArtisanCommand {
  /** `posts:publish`. */
  name: string;
  description: string;
  /** `posts:publish [--dry-run] <post>`. */
  synopsis: string;
  arguments: ArtisanArgument[];
  options: ArtisanOption[];
  source: "class" | "closure" | "package" | "framework";
  class: string | null;
  location: Place | null;
}

/** The command needs something typed before it can run. */
export const needsArguments = (command: ArtisanCommand) => command.arguments.some((a) => a.required);

/** What the arguments box says about `command`: its arguments, then its options. */
export function argumentsHelp(command: ArtisanCommand): string {
  const parts: string[] = [];
  for (const a of command.arguments) {
    const name = a.required ? `<${a.name}>` : `[${a.name}]`;
    parts.push(a.description ? `${name}: ${a.description}` : name);
  }
  for (const o of command.options) {
    const name = `--${o.name}${o.acceptsValue ? "=" : ""}`;
    parts.push(o.description ? `${name}: ${o.description}` : name);
  }
  return parts.join(" · ");
}

/** The command line running Artisan command `name` with `args` through `artisan` (`php artisan`). */
export function artisanCommandLine(artisan: string, name: string, args = ""): string {
  return [artisan.trim() || "php artisan", name.trim(), args.trim()].filter(Boolean).join(" ");
}

// ---------------------------------------------------------------------------
// "Create translation key…".

export interface LocaleText {
  locale: string;
  /** The locale's text when it has the key, else the suggested one. */
  text: string;
  /** The locale has the key already (it is left as it is). */
  exists: boolean;
}

/** What the action asks about: the key and every locale of the project. */
export interface TranslationRequest {
  key: string;
  locales: LocaleText[];
}

/** The request of the action's command argument (its JSON), or null. */
export function translationRequestOf(arg: unknown): TranslationRequest | null {
  let value: unknown = arg;
  if (typeof arg === "string") {
    try {
      value = JSON.parse(arg);
    } catch {
      return null;
    }
  }
  const r = value as Partial<TranslationRequest> | null;
  if (!r || typeof r.key !== "string" || !Array.isArray(r.locales)) return null;
  const locales = r.locales.filter(
    (l): l is LocaleText => !!l && typeof l.locale === "string" && typeof l.text === "string",
  );
  return { key: r.key, locales: locales.map((l) => ({ locale: l.locale, text: l.text, exists: !!l.exists })) };
}

/** The locales the key can be added to. */
export const missingLocales = (request: TranslationRequest) => request.locales.filter((l) => !l.exists);

// ---------------------------------------------------------------------------
// Edits and templates.

/** An edit the server made for the client to apply. */
export interface Applied {
  /** An LSP `WorkspaceEdit`. */
  edit: unknown;
  /** The URIs of the files it creates, in order. */
  created: string[];
}

/** A generator template. */
export interface Template {
  /** `Laravel Model`. */
  name: string;
  /** Where the user's version is (or would be). */
  path: string;
  body: string;
  edited: boolean;
}
