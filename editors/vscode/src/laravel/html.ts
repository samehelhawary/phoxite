// The HTML of the Laravel pages (the Eloquent model view, the New Model form).
// No `vscode` here: the tests run this in plain Node. The pages' scripts and
// styles are files of `media/`.

import {
  COLUMN_TYPES,
  FOREIGN_TYPES,
  MODEL_COLUMN_TYPES,
  ON_DELETE,
  CONTROLLER_KINDS,
  type GenerationContext,
  type ModelColumn,
  type ModelView,
  type NewModelForm,
  type Place,
  RELATION_KINDS,
  shortName,
} from "./model.ts";

/** Where a page's resources are, as the webview addresses them. */
export interface PageResources {
  /** The webview's own origin, for the content security policy. */
  cspSource: string;
  style: string;
  script: string;
}

const ESCAPES: Record<string, string> = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" };

/** `text` as HTML text or an attribute value. */
export const escapeHtml = (text: string) => text.replace(/[&<>"']/g, (c) => ESCAPES[c]!);

/** `value` as JSON inside a `<script type="application/json">`. */
export const embedJson = (value: unknown) => JSON.stringify(value).replace(/</g, "\\u003c");

/** A page: only its own style and script load, and nothing from the network. */
export function page(title: string, resources: PageResources, body: string, data?: unknown): string {
  const csp = `default-src 'none'; style-src ${resources.cspSource}; script-src ${resources.cspSource};`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${escapeHtml(csp)}">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>${escapeHtml(title)}</title>
<link rel="stylesheet" href="${escapeHtml(resources.style)}">
</head>
<body>
${body}
${data === undefined ? "" : `<script type="application/json" id="data">${embedJson(data)}</script>`}
${resources.script ? `<script src="${escapeHtml(resources.script)}"></script>` : ""}
</body>
</html>`;
}

/** A page that only says something: no script runs in it. */
export function messagePage(title: string, text: string, resources: PageResources): string {
  return page(title, { ...resources, script: "" }, `<main><p class="hint" role="status">${escapeHtml(text)}</p></main>`);
}

/**
 * What a panel's page keeps with `setState`, so that the panel opens again after a
 * window reload: enough to ask the server for the rest.
 */
export type PanelState = Record<string, unknown>;

// ---------------------------------------------------------------------------
// The model view.

/** What the context menu of a model view item carries (`data-vscode-context`). */
export interface ItemContext {
  webviewSection: "item";
  place: Place;
  renamable: boolean;
  preventDefaultContextMenuItems: true;
}

/** An item's name: a link going to it, with Find Usages and Rename in its context menu. */
function item(label: string, at: Place | null, renamable = true): string {
  if (!at) return `<span class="name">${escapeHtml(label)}</span>`;
  const context: ItemContext = { webviewSection: "item", place: at, renamable, preventDefaultContextMenuItems: true };
  return `<a href="#" class="link" data-place="${escapeHtml(JSON.stringify(at))}" data-vscode-context="${escapeHtml(JSON.stringify(context))}">${escapeHtml(label)}</a>`;
}

/** Where a column comes from. */
export function columnSource(c: ModelColumn): string {
  if (c.source !== "database") return "migration";
  return c.database ? `database ${c.database}` : "database";
}

function section(title: string, count: number, body: string): string {
  return `<section aria-label="${escapeHtml(title)}">
<h2>${escapeHtml(title)} <span class="count">${count}</span></h2>
${count ? body : `<p class="hint">None</p>`}
</section>`;
}

const RELATED_LABELS: Record<string, string> = {
  factory: "Factory",
  resource: "Resource",
  policy: "Policy",
  observer: "Observer",
  migration: "Migration",
};

const list = (items: string[]) => `<ul>${items.map((i) => `<li>${i}</li>`).join("")}</ul>`;
const detail = (text: string) => (text.trim() ? ` <span class="detail">${escapeHtml(text.trim())}</span>` : "");

/** What the model view shows of `view`: the part replaced when the project changes. */
export function modelDetails(view: ModelView): string {
  const columns = view.columns
    .map(
      (c) => `<tr>
<td>${item(c.name, c.location, false)}</td>
<td><code>${escapeHtml(c.nullable ? `?${c.ty}` : c.ty)}</code></td>
<td>${c.cast ? `<code>${escapeHtml(c.cast.replace(/^\\/, ""))}</code>` : ""}</td>
<td>${escapeHtml(columnSource(c))}</td>
<td class="detail">${[c.fillable && "fillable", c.hidden && "hidden"].filter(Boolean).join(", ")}</td>
</tr>`,
    )
    .join("");
  return `<div class="summary">
<span><span class="label">Class:</span> ${item(view.class, view.location)}</span>
<span><span class="label">Table:</span> <code>${escapeHtml(view.table)}</code></span>
<span><span class="label">Primary key:</span> <code>${escapeHtml(`${view.primaryKey} (${view.keyType})`)}</code>${view.incrementing ? detail("auto-increment") : ""}</span>
</div>
${view.schemaCertain ? "" : `<p class="hint">The columns may be incomplete: no migration creating the table was fully understood.</p>`}
${section(
  "Columns",
  view.columns.length,
  `<table><thead><tr><th>Name</th><th>Type</th><th>Cast</th><th>Source</th><th>Attributes</th></tr></thead><tbody>${columns}</tbody></table>`,
)}
${section(
  "Relations",
  view.relations.length,
  list(
    view.relations.map(
      (r) => `${item(r.name, r.location)}${detail(`${r.kind}${r.related ? ` → ${shortName(r.related)}` : ""}`)}`,
    ),
  ),
)}
${section("Scopes", view.scopes.length, list(view.scopes.map((s) => `${item(s.name, s.location)}${detail(`${s.method}()`)}`)))}
${section(
  "Accessors",
  view.accessors.length,
  list(view.accessors.map((a) => `${item(a.name, a.location)}${detail(`${a.ty ?? ""} ${a.method}()`)}`)),
)}
${section(
  "Related",
  view.related.length,
  list(
    view.related.map(
      (r) =>
        `<span class="kind">${escapeHtml(RELATED_LABELS[r.kind] ?? r.kind)}</span> ${item(r.kind === "migration" ? r.name : shortName(r.name), r.location, r.kind !== "migration")}`,
    ),
  ),
)}`;
}

/** The model view's page: the details, and the Add Columns form its script runs. */
export function modelViewPage(view: ModelView, resources: PageResources, panel?: PanelState): string {
  const body = `<main>
<div id="details">${modelDetails(view)}</div>
<p class="buttons" id="open-form"><button type="button" data-action="show-form">Add Columns…</button></p>
<form id="add-columns" class="hidden" aria-label="Add Columns">
<h2>Add Columns</h2>
<p class="hint">A new migration adds the columns to the table.</p>
<div id="rows"></div>
<p class="buttons"><button type="button" class="secondary" data-action="add-row">Add Row</button></p>
<label class="check"><input type="checkbox" id="fillable" checked> Add them to <code>$fillable</code></label>
<p class="error hidden" id="error" role="alert"></p>
<p class="buttons">
<button type="submit" title="Ctrl+Enter">Create Migration</button>
<button type="button" class="secondary" data-action="hide-form">Cancel</button>
</p>
</form>
</main>`;
  return page(`Eloquent Model ${view.name}`, resources, body, { columnTypes: COLUMN_TYPES, panel });
}

/** The page shown when the model can't be read. */
export function modelErrorPage(cls: string, error: string, resources: PageResources, panel?: PanelState): string {
  return page(
    `Eloquent Model ${shortName(cls)}`,
    resources,
    `<main><div id="details"><p class="error" role="alert">${escapeHtml(error)}</p></div></main>`,
    { columnTypes: COLUMN_TYPES, panel },
  );
}

// ---------------------------------------------------------------------------
// The New Model form.

/** What the New Model page's script builds the form from. */
export interface NewModelData {
  namespace: string;
  modules: string[];
  models: string[];
  columnTypes: readonly string[];
  onDelete: ReadonlyArray<{ value: string; label: string }>;
  foreignTypes: readonly string[];
  relationKinds: readonly string[];
  controllerKinds: ReadonlyArray<{ value: string; label: string }>;
  form: NewModelForm;
  panel?: PanelState;
}

export function newModelPage(
  ctx: GenerationContext,
  form: NewModelForm,
  resources: PageResources,
  panel?: PanelState,
): string {
  const data: NewModelData = {
    namespace: ctx.namespace,
    modules: ctx.modules,
    models: ctx.models,
    columnTypes: MODEL_COLUMN_TYPES,
    onDelete: ON_DELETE,
    foreignTypes: FOREIGN_TYPES,
    relationKinds: RELATION_KINDS,
    controllerKinds: CONTROLLER_KINDS,
    form,
    panel,
  };
  const body = `<main>
<form id="new-model" aria-label="New Eloquent Model">
<h1>New Eloquent Model</h1>
<div class="row" id="head"></div>
<h2>Fields</h2>
<div id="fields"></div>
<p class="buttons"><button type="button" class="secondary" data-action="add-field">Add Field</button></p>
<h2>Relations</h2>
<div id="relations"></div>
<p class="buttons"><button type="button" class="secondary" data-action="add-relation">Add Relation</button></p>
<h2>Indexes</h2>
<div id="indexes"></div>
<p class="buttons"><button type="button" class="secondary" data-action="add-index">Add Index</button></p>
<h2>Generate</h2>
<div class="checks" id="generate"></div>
<p class="error hidden" id="error" role="alert"></p>
<p class="buttons">
<button type="submit" id="create" title="Ctrl+Enter">Create</button>
<button type="button" class="secondary" data-action="cancel">Cancel</button>
</p>
</form>
</main>`;
  return page("New Eloquent Model", resources, body, data);
}
