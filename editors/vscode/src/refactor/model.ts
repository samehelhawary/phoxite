// The refactorings that ask something first (Change Signature, Extract Method,
// the Generate member choosers, Pull Members Up / Push Members Down): what the
// server sends, and the rules of their dialogs. No `vscode` here: the tests run
// this in plain Node.

/** An engine position: 0-based line and UTF-16 column. */
export interface Pos {
  line: number;
  col: number;
}

// ---------------------------------------------------------------------------
// Change Signature.

export interface SignatureParameter {
  /** Its position in the current signature; `null` for a new parameter. */
  oldIndex: number | null;
  /** Without the `$`. */
  name: string;
  type: string;
  defaultValue: string;
  /** For a new parameter: the argument existing calls pass. */
  callValue: string;
  byRef: boolean;
  variadic: boolean;
  /** A promoted constructor parameter: it can't be renamed or removed. */
  promoted: boolean;
}

export interface Signature {
  /** `name` or `Class::name`, for display. */
  name: string;
  functionName: string;
  /** `public`, `protected`, `private`; empty for a function. */
  visibility: string;
  parameters: SignatureParameter[];
  returnType: string;
  constructor: boolean;
}

/** What the "Change signature" action carries; the dialog fills in `change`. */
export interface ChangeSignatureData {
  position: Pos;
  signature: Signature;
  change?: Signature | null;
}

export const VISIBILITIES = ["public", "protected", "private"];

const IDENTIFIER = /^[A-Za-z_\u0080-￿][A-Za-z0-9_\u0080-￿]*$/;

/** Whether `name` can name a PHP function, method or variable (without `$`). */
export const isIdentifier = (name: string) => IDENTIFIER.test(name);

/** A parameter name as typed: without the `$` and the spaces around it. */
export const bareName = (name: string) => name.trim().replace(/^\$/, "");

export function newParameter(): SignatureParameter {
  return {
    oldIndex: null,
    name: "",
    type: "",
    defaultValue: "",
    callValue: "",
    byRef: false,
    variadic: false,
    promoted: false,
  };
}

/** The signature as the dialog's fields give it: names and types trimmed. */
export function tidySignature(s: Signature): Signature {
  return {
    ...s,
    functionName: s.functionName.trim(),
    returnType: s.constructor ? "" : s.returnType.trim(),
    parameters: s.parameters.map((p) => ({
      ...p,
      name: bareName(p.name),
      type: p.type.trim(),
      defaultValue: p.defaultValue.trim(),
      callValue: p.oldIndex === null ? p.callValue.trim() : "",
    })),
  };
}

/** Why a (tidied) signature can't be applied; `undefined` when it can. */
export function signatureError(was: Signature, s: Signature): string | undefined {
  if (!isIdentifier(s.functionName)) return `'${s.functionName}' is not a valid name`;
  const seen = new Set<string>();
  for (const [i, p] of s.parameters.entries()) {
    if (!p.name) return `Parameter ${i + 1} needs a name`;
    if (!isIdentifier(p.name)) return `'$${p.name}' is not a valid parameter name`;
    if (seen.has(p.name)) return `Two parameters are named '$${p.name}'`;
    seen.add(p.name);
    if (p.variadic && i !== s.parameters.length - 1) return `'...$${p.name}' has to be the last parameter`;
    if (p.variadic && p.defaultValue) return `'...$${p.name}' can't have a default value`;
  }
  for (const [index, old] of was.parameters.entries()) {
    if (!old.promoted) continue;
    const kept = s.parameters.find((p) => p.oldIndex === index);
    if (!kept) return `'$${old.name}' is a promoted property: it can't be removed here`;
    if (kept.name !== old.name) return `'$${old.name}' is a promoted property: rename it with Rename Symbol`;
  }
  if (JSON.stringify(tidySignature(was)) === JSON.stringify(s)) return "Nothing was changed";
  return undefined;
}

/** `public function total(int $a, int ...$rest): int`. */
export function signaturePreview(s: Signature): string {
  const parameters = s.parameters.map((p) => {
    const name = `${p.byRef ? "&" : ""}${p.variadic ? "..." : ""}$${p.name}`;
    return `${p.type ? `${p.type} ` : ""}${name}${p.defaultValue ? ` = ${p.defaultValue}` : ""}`;
  });
  const head = `${s.visibility ? `${s.visibility} ` : ""}function ${s.functionName}(${parameters.join(", ")})`;
  return s.returnType && !s.constructor ? `${head}: ${s.returnType}` : head;
}

// ---------------------------------------------------------------------------
// Extract Method.

export interface ExtractParameter {
  oldName: string;
  name: string;
  type: string;
  included: boolean;
}

export interface ExtractChoice {
  name: string;
  parameters: ExtractParameter[];
  returnType: string;
  visibility: string;
  static: boolean;
  replaceDuplicates: boolean;
}

/** What the "Extract method…" action carries; the dialog fills in `change`. */
export interface ExtractMethodData {
  range: { start: Pos; end: Pos };
  /** A function, not a method. */
  function: boolean;
  className: string;
  defaults: ExtractChoice;
  staticRequired: boolean;
  usesThis: boolean;
  duplicates: number;
  change?: ExtractChoice | null;
}

/** One line of the Extract Method options list. */
export interface ExtractOption {
  id: string;
  label: string;
  description?: string;
  picked: boolean;
}

const PARAMETER = "parameter:";

/** The options of an extraction: what to pass, `static`, and the duplicates. */
export function extractOptions(data: ExtractMethodData): ExtractOption[] {
  const options: ExtractOption[] = data.defaults.parameters.map((p) => ({
    id: PARAMETER + p.oldName,
    label: `Pass $${p.oldName}`,
    description: p.type || undefined,
    picked: p.included,
  }));
  if (!data.function && !data.staticRequired && !data.usesThis) {
    options.push({ id: "static", label: "Static", picked: data.defaults.static });
  }
  if (data.duplicates > 0) {
    options.push({
      id: "duplicates",
      label: data.duplicates === 1 ? "Replace the duplicate" : `Replace the ${data.duplicates} duplicates`,
      picked: data.defaults.replaceDuplicates,
    });
  }
  return options;
}

/** The choice of the dialog: the name, the visibility and the options ticked. */
export function extractChoice(data: ExtractMethodData, name: string, visibility: string, picked: string[]): ExtractChoice {
  const d = data.defaults;
  const offered = new Set(extractOptions(data).map((o) => o.id));
  const chosen = (id: string, otherwise: boolean) => (offered.has(id) ? picked.includes(id) : otherwise);
  return {
    ...d,
    name: name.trim(),
    visibility: data.function ? d.visibility : visibility,
    parameters: d.parameters.map((p) => ({ ...p, included: picked.includes(PARAMETER + p.oldName) })),
    static: chosen("static", d.static),
    replaceDuplicates: chosen("duplicates", false),
  };
}

/** Why `name` can't be the new method's or function's name. */
export function extractNameError(name: string): string | undefined {
  const trimmed = name.trim();
  if (!trimmed) return "Enter a name";
  return isIdentifier(trimmed) ? undefined : `'${trimmed}' is not a valid name`;
}

// ---------------------------------------------------------------------------
// The Generate member choosers.

export interface ChooserMember {
  id: string;
  label: string;
  detail: string;
  selected: boolean;
}

/** What a "Generate …" action with a chooser carries; the dialog fills in `chosen`. */
export interface MemberChooserData {
  range: { start: Pos; end: Pos };
  generator: string;
  title: string;
  members: ChooserMember[];
  chosen?: string[] | null;
}

// ---------------------------------------------------------------------------
// Pull Members Up / Push Members Down.

export type HierarchyKind = "pullUp" | "pushDown";

export interface HierarchyTarget {
  id: string;
  name: string;
  qualifiedName: string;
  interface: boolean;
}

export interface Movable {
  id: string;
  label: string;
  kind: "field" | "method";
  selected: boolean;
  abstractable: boolean;
}

export interface HierarchyInfo {
  className: string;
  targets: HierarchyTarget[];
  members: Movable[];
}

export interface HierarchyChoice {
  id: string;
  makeAbstract: boolean;
}

export interface HierarchyProblem {
  severity: "info" | "warning" | "error" | "fatal";
  message: string;
}

/** The members chosen, with those of `abstracted` only declared abstract. */
export function hierarchyChoices(info: HierarchyInfo, moved: string[], abstracted: string[]): HierarchyChoice[] {
  return info.members
    .filter((m) => moved.includes(m.id))
    .map((m) => ({ id: m.id, makeAbstract: m.abstractable && abstracted.includes(m.id) }));
}

/**
 * What the question about abstract methods says, for each direction; `undefined`
 * when there is nothing to ask (an interface only declares its methods anyway).
 */
export function abstractPrompt(kind: HierarchyKind, target: HierarchyTarget | undefined): string | undefined {
  if (kind === "pushDown") return "Methods to keep as abstract declarations in this class (none: Enter)";
  if (target?.interface) return undefined;
  return "Methods to leave here and declare abstract in the parent (none: Enter)";
}

/** The problems that stop a move, and those the user confirms first. */
export function splitProblems(problems: HierarchyProblem[]): { fatal: string[]; confirm: string[] } {
  return {
    fatal: problems.filter((p) => p.severity === "fatal").map((p) => p.message),
    confirm: problems.filter((p) => p.severity !== "fatal" && p.severity !== "info").map((p) => p.message),
  };
}
