import assert from "node:assert/strict";
import { test } from "node:test";
import { changeSignaturePage } from "../src/refactor/html.ts";
import {
  type ExtractMethodData,
  type HierarchyInfo,
  type Signature,
  type SignatureParameter,
  abstractPrompt,
  extractChoice,
  extractNameError,
  extractOptions,
  hierarchyChoices,
  newParameter,
  signatureError,
  signaturePreview,
  splitProblems,
  tidySignature,
} from "../src/refactor/model.ts";

const parameter = (oldIndex: number | null, name: string, type = "", more: Partial<SignatureParameter> = {}) => ({
  ...newParameter(),
  oldIndex,
  name,
  type,
  ...more,
});

const total: Signature = {
  name: "Shop::total",
  functionName: "total",
  visibility: "public",
  parameters: [parameter(0, "a", "int"), parameter(1, "b", "int")],
  returnType: "int",
  constructor: false,
};

test("a signature from the form is tidied", () => {
  const typed: Signature = {
    ...total,
    functionName: " sum ",
    returnType: " int ",
    parameters: [parameter(0, " $a ", " int ", { callValue: "ignored" }), parameter(null, "$c", "", { callValue: " 0 " })],
  };
  const tidy = tidySignature(typed);
  assert.equal(tidy.functionName, "sum");
  assert.equal(tidy.returnType, "int");
  assert.deepEqual(
    tidy.parameters.map((p) => [p.name, p.type, p.callValue]),
    [
      ["a", "int", ""],
      ["c", "", "0"],
    ],
  );
  assert.equal(tidySignature({ ...total, constructor: true }).returnType, "");
});

test("a signature is checked before it is sent", () => {
  const change = (more: Partial<Pick<Signature, "functionName" | "parameters">>) =>
    signatureError(total, tidySignature({ ...total, ...more }));
  assert.equal(change({}), "Nothing was changed");
  assert.equal(change({ functionName: "sum" }), undefined);
  assert.equal(change({ parameters: [...total.parameters].reverse() }), undefined);
  assert.equal(change({ functionName: "2fast" }), "'2fast' is not a valid name");
  assert.equal(change({ parameters: [parameter(0, "a"), parameter(null, "")] }), "Parameter 2 needs a name");
  assert.equal(change({ parameters: [parameter(0, "a-b")] }), "'$a-b' is not a valid parameter name");
  assert.equal(change({ parameters: [parameter(0, "a"), parameter(1, "a")] }), "Two parameters are named '$a'");
  assert.equal(
    change({ parameters: [parameter(0, "a", "", { variadic: true }), parameter(1, "b")] }),
    "'...$a' has to be the last parameter",
  );
  assert.equal(
    change({ parameters: [parameter(0, "a", "", { variadic: true, defaultValue: "[]" })] }),
    "'...$a' can't have a default value",
  );
});

test("a promoted property stays in the signature under its name", () => {
  const was: Signature = {
    ...total,
    name: "Shop::__construct",
    functionName: "__construct",
    constructor: true,
    returnType: "",
    parameters: [parameter(0, "name", "string", { promoted: true }), parameter(1, "limit", "int")],
  };
  const change = (parameters: SignatureParameter[]) => signatureError(was, tidySignature({ ...was, parameters }));
  assert.match(change([parameter(1, "limit", "int")]) ?? "", /promoted property: it can't be removed/);
  assert.match(change([parameter(0, "title", "string", { promoted: true })]) ?? "", /rename it with Rename Symbol/);
  assert.equal(change([parameter(1, "limit", "int"), parameter(0, "name", "string", { promoted: true })]), undefined);
});

test("the preview reads as PHP", () => {
  assert.equal(signaturePreview(total), "public function total(int $a, int $b): int");
  const fancy: Signature = {
    ...total,
    visibility: "",
    functionName: "collect",
    returnType: "",
    parameters: [
      parameter(0, "into", "array", { byRef: true }),
      parameter(1, "sep", "string", { defaultValue: "','" }),
      parameter(2, "rest", "", { variadic: true }),
    ],
  };
  assert.equal(signaturePreview(fancy), "function collect(array &$into, string $sep = ',', ...$rest)");
});

test("the Change Signature page carries the signature and loads only its own files", () => {
  const resources = { cspSource: "vscode-resource:", style: "s.css", script: "s.js" };
  const html = changeSignaturePage({ ...total, name: "Shop::<total>" }, resources);
  assert.match(html, /<h1>Change Signature of Shop::&lt;total&gt;<\/h1>/);
  assert.match(html, /default-src &#39;none&#39;/);
  const data = JSON.parse(/<script type="application\/json" id="data">(.*?)<\/script>/s.exec(html)![1]);
  assert.equal(data.signature.parameters.length, 2);
  assert.deepEqual(data.visibilities, ["public", "protected", "private"]);
  assert.ok(!html.includes("<total>"));
});

const extraction: ExtractMethodData = {
  range: { start: { line: 1, col: 0 }, end: { line: 1, col: 9 } },
  function: false,
  className: "Shop",
  defaults: {
    name: "extracted",
    parameters: [
      { oldName: "a", name: "a", type: "int", included: true },
      { oldName: "b", name: "b", type: "", included: true },
    ],
    returnType: "int",
    visibility: "private",
    static: false,
    replaceDuplicates: true,
  },
  staticRequired: false,
  usesThis: false,
  duplicates: 2,
};

test("the Extract Method options are what the selection allows", () => {
  assert.deepEqual(
    extractOptions(extraction).map((o) => [o.id, o.label, o.picked]),
    [
      ["parameter:a", "Pass $a", true],
      ["parameter:b", "Pass $b", true],
      ["static", "Static", false],
      ["duplicates", "Replace the 2 duplicates", true],
    ],
  );
  assert.equal(extractOptions(extraction)[0]!.description, "int");
  // Code using `$this` can't be static; a static context has to be.
  const ids = (data: ExtractMethodData) => extractOptions(data).map((o) => o.id);
  assert.ok(!ids({ ...extraction, usesThis: true }).includes("static"));
  assert.ok(!ids({ ...extraction, staticRequired: true }).includes("static"));
  assert.deepEqual(ids({ ...extraction, function: true, duplicates: 0 }), ["parameter:a", "parameter:b"]);
  assert.equal(extractOptions({ ...extraction, duplicates: 1 }).at(-1)!.label, "Replace the duplicate");
});

test("the Extract Method choice is the defaults with the answers", () => {
  const choice = extractChoice(extraction, " add ", "protected", ["parameter:a", "static"]);
  assert.equal(choice.name, "add");
  assert.equal(choice.visibility, "protected");
  assert.deepEqual(
    choice.parameters.map((p) => p.included),
    [true, false],
  );
  assert.equal(choice.static, true);
  assert.equal(choice.replaceDuplicates, false);
  assert.equal(choice.returnType, "int");
  // What isn't offered keeps its default: a required `static`, a function's visibility.
  const required = { ...extraction, staticRequired: true, defaults: { ...extraction.defaults, static: true } };
  assert.equal(extractChoice(required, "add", "private", []).static, true);
  const fn = { ...extraction, function: true, defaults: { ...extraction.defaults, visibility: "" } };
  assert.equal(extractChoice(fn, "add", "public", []).visibility, "");
  assert.equal(extractNameError(" "), "Enter a name");
  assert.equal(extractNameError("do-it"), "'do-it' is not a valid name");
  assert.equal(extractNameError(" add "), undefined);
});

const hierarchy: HierarchyInfo = {
  className: "Shop",
  targets: [
    { id: "App\\Base", name: "Base", qualifiedName: "App\\Base", interface: false },
    { id: "App\\Sells", name: "Sells", qualifiedName: "App\\Sells", interface: true },
  ],
  members: [
    { id: "method:total", label: "total(int $a): int", kind: "method", selected: true, abstractable: true },
    { id: "method:secret", label: "secret()", kind: "method", selected: false, abstractable: false },
    { id: "property:count", label: "$count: int", kind: "field", selected: false, abstractable: false },
  ],
};

test("the members to move, and those only declared abstract", () => {
  assert.deepEqual(hierarchyChoices(hierarchy, ["method:total", "method:secret"], ["method:total", "method:secret"]), [
    { id: "method:total", makeAbstract: true },
    { id: "method:secret", makeAbstract: false },
  ]);
  assert.deepEqual(hierarchyChoices(hierarchy, ["property:count"], []), [{ id: "property:count", makeAbstract: false }]);
  assert.match(abstractPrompt("pullUp", hierarchy.targets[0]) ?? "", /declare abstract in the parent/);
  assert.equal(abstractPrompt("pullUp", hierarchy.targets[1]), undefined);
  assert.match(abstractPrompt("pushDown", undefined) ?? "", /keep as abstract declarations/);
});

test("fatal problems stop a move, warnings are confirmed, notes pass", () => {
  const split = splitProblems([
    { severity: "info", message: "A note." },
    { severity: "warning", message: "Shop becomes abstract." },
    { severity: "error", message: "total() uses $count, which stays." },
    { severity: "fatal", message: "Base already declares total()." },
  ]);
  assert.deepEqual(split.fatal, ["Base already declares total()."]);
  assert.deepEqual(split.confirm, ["Shop becomes abstract.", "total() uses $count, which stays."]);
});
