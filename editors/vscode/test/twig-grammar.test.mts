// The Twig grammar, tokenized as VS Code does (vscode-textmate with the Oniguruma
// engine). It builds on VS Code's own HTML grammar, which is taken from the VS Code
// that `npm run test:e2e` downloads; without one the tests are skipped.

import assert from "node:assert/strict";
import * as fs from "node:fs";
import { createRequire } from "node:module";
import * as path from "node:path";
import { test } from "node:test";

// The tests run from the extension's folder (`npm test`).
const extension = process.cwd();
const require = createRequire(path.join(extension, "package.json"));

/** The `extensions` folder of a downloaded VS Code, if there is one. */
function builtinExtensions(): string | undefined {
  const root = path.join(extension, ".vscode-test");
  if (!fs.existsSync(root)) return undefined;
  const found: string[] = [];
  const visit = (dir: string, depth: number) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (!entry.isDirectory()) continue;
      const full = path.join(dir, entry.name);
      if (entry.name === "extensions" && fs.existsSync(path.join(full, "html", "syntaxes", "html.tmLanguage.json"))) {
        found.push(full);
      } else if (depth < 6 && entry.name !== "node_modules") {
        visit(full, depth + 1);
      }
    }
  };
  visit(root, 0);
  return found[0];
}

const builtin = builtinExtensions();
const skip = builtin ? false : "no downloaded VS Code under .vscode-test (run `npm run test:e2e` once)";

type Token = { text: string; scopes: string[] };

async function tokenizer(): Promise<(source: string) => Token[][]> {
  const textmate = require("vscode-textmate");
  const oniguruma = require("vscode-oniguruma");
  const wasm = fs.readFileSync(require.resolve("vscode-oniguruma/release/onig.wasm"));
  await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const files: Record<string, string> = {
    "text.html.twig": path.join(extension, "syntaxes", "twig.tmLanguage.json"),
    "text.html.basic": path.join(builtin!, "html", "syntaxes", "html.tmLanguage.json"),
    "text.html.derivative": path.join(builtin!, "html", "syntaxes", "html-derivative.tmLanguage.json"),
    "source.js": path.join(builtin!, "javascript", "syntaxes", "JavaScript.tmLanguage.json"),
    "source.css": path.join(builtin!, "css", "syntaxes", "css.tmLanguage.json"),
  };
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (patterns: string[]) => new oniguruma.OnigScanner(patterns),
      createOnigString: (text: string) => new oniguruma.OnigString(text),
    }),
    loadGrammar: async (scope: string) =>
      files[scope] && fs.existsSync(files[scope])
        ? textmate.parseRawGrammar(fs.readFileSync(files[scope], "utf8"), files[scope])
        : null,
  });
  const grammar = await registry.loadGrammar("text.html.twig");
  return (source: string) => {
    let stack = textmate.INITIAL;
    return source.split("\n").map((line) => {
      const result = grammar.tokenizeLine(line, stack);
      stack = result.ruleStack;
      return result.tokens.map((t: { startIndex: number; endIndex: number; scopes: string[] }) => ({
        text: line.slice(t.startIndex, t.endIndex),
        scopes: t.scopes,
      }));
    });
  };
}

/** The scopes of the first token whose text is `text`. */
function scopesOf(lines: Token[][], text: string): string[] {
  const token = lines.flat().find((t) => t.text === text);
  assert.ok(token, `no token ${JSON.stringify(text)} among ${JSON.stringify(lines.flat().map((t) => t.text))}`);
  return token.scopes;
}

const has = (scopes: string[], scope: string) => scopes.some((s) => s === scope || s.startsWith(`${scope}.`));

test("an output in the text of a template: a variable, a property and a filter", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize("<p>{{ post.title|upper }}</p>");
  assert.ok(has(scopesOf(lines, "{{"), "punctuation.section.embedded.begin.twig"));
  assert.ok(has(scopesOf(lines, "post"), "variable.other.twig"));
  assert.ok(has(scopesOf(lines, "title"), "variable.other.property.twig"));
  assert.ok(has(scopesOf(lines, "upper"), "support.function.filter.twig"));
  // The rest is HTML as before.
  assert.ok(has(scopesOf(lines, "p"), "entity.name.tag"));
});

test("a statement: its tag is a keyword, then an expression", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize("{% for item in items if item.active is not empty %}x{% endfor %}");
  assert.ok(has(scopesOf(lines, "for"), "keyword.control.twig"));
  assert.ok(has(scopesOf(lines, "endfor"), "keyword.control.twig"));
  assert.ok(has(scopesOf(lines, "in"), "keyword.operator.word.twig"));
  assert.ok(has(scopesOf(lines, "empty"), "support.function.test.twig"));
  assert.ok(has(scopesOf(lines, "items"), "variable.other.twig"));
  assert.ok(!has(scopesOf(lines, "x"), "meta.statement.twig"), "the body is text");
});

test("strings, numbers, functions and whitespace control", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`{%- set label = path('home', { page: 2 }) ~ "a #{name} b" -%}`);
  assert.ok(has(scopesOf(lines, "{%-"), "punctuation.section.embedded.begin.twig"));
  assert.ok(has(scopesOf(lines, "-%}"), "punctuation.section.embedded.end.twig"));
  assert.ok(has(scopesOf(lines, "path"), "entity.name.function.twig"));
  assert.ok(has(scopesOf(lines, "home"), "string.quoted.single.twig"));
  assert.ok(has(scopesOf(lines, "2"), "constant.numeric.twig"));
  const interpolated = scopesOf(lines, "name");
  assert.ok(has(interpolated, "meta.interpolation.twig") && has(interpolated, "variable.other.twig"));
});

test("a comment, and a verbatim block whose content is not Twig", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize("{# note {{ x }} #}\n{% verbatim %}{{ raw }}{% endverbatim %}");
  assert.ok(lines[0].every((t) => has(t.scopes, "comment.block.twig")));
  assert.ok(!lines[1].some((t) => has(t.scopes, "meta.output.twig")), "{{ raw }} is text");
  assert.ok(has(scopesOf(lines, "endverbatim"), "keyword.control.twig"));
});

test("an output inside an attribute value is Twig, not attribute text", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`<a href="{{ path('post', { id: post.id }) }}" class="link">x</a>`);
  const call = scopesOf(lines, "path");
  assert.ok(has(call, "entity.name.function.twig") && has(call, "meta.tag"), call.join(" "));
  assert.ok(has(scopesOf(lines, "class"), "entity.other.attribute-name"));
  assert.ok(has(scopesOf(lines, "link"), "string.quoted.double.html"));
});

test("a statement inside a tag, and an output in a script", { skip }, async () => {
  const tokenize = await tokenizer();
  const inTag = tokenize(`<li {% if active %}class="on"{% endif %}>`);
  assert.ok(has(scopesOf(inTag, "if"), "keyword.control.twig"));
  assert.ok(has(scopesOf(inTag, "class"), "entity.other.attribute-name"));
  const inScript = tokenize(["<script>", "  const id = {{ post.id }};", "</script>"].join("\n"));
  assert.ok(has(scopesOf(inScript, "post"), "variable.other.twig"));
});
