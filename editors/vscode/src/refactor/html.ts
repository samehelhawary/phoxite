// The HTML of the Change Signature form. No `vscode` here: the tests run this in
// plain Node. Its script and style are files of `media/`.

import { type PageResources, type PanelState, escapeHtml, page } from "../laravel/html.ts";
import { type Signature, VISIBILITIES } from "./model.ts";

/** What the form's script reads. */
export interface ChangeSignatureFormData {
  signature: Signature;
  visibilities: string[];
  panel?: PanelState;
}

export function changeSignaturePage(signature: Signature, resources: PageResources, panel?: PanelState): string {
  const data: ChangeSignatureFormData = { signature, visibilities: VISIBILITIES, panel };
  const title = `Change Signature of ${signature.name}`;
  const body = `<main>
<form id="signature" aria-label="${escapeHtml(title)}">
<h1>${escapeHtml(title)}</h1>
<div class="row" id="head"></div>
<h2>Parameters</h2>
<div id="parameters"></div>
<p class="hint hidden" id="none">No parameters.</p>
<p class="buttons"><button type="button" class="secondary" data-action="add">Add Parameter</button></p>
<h2>Preview</h2>
<p><code id="preview"></code></p>
<p class="error hidden" id="error" role="alert"></p>
<p class="buttons">
<button type="submit" id="apply" title="Ctrl+Enter">Refactor</button>
<button type="button" class="secondary" data-action="cancel">Cancel</button>
</p>
</form>
</main>`;
  return page(title, resources, body, data);
}
