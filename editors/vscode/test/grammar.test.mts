// The Blade grammar, tokenized as VS Code does (vscode-textmate with the Oniguruma
// engine). It builds on VS Code's own HTML and PHP grammars, which are taken from the
// VS Code that `npm run test:e2e` downloads; without one the tests are skipped.

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
      if (entry.name === "extensions" && fs.existsSync(path.join(full, "php", "syntaxes", "php.tmLanguage.json"))) {
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
    "text.html.php.blade": path.join(extension, "syntaxes", "blade.tmLanguage.json"),
    "text.html.php": path.join(builtin!, "php", "syntaxes", "html.tmLanguage.json"),
    "source.php": path.join(builtin!, "php", "syntaxes", "php.tmLanguage.json"),
    "text.html.basic": path.join(builtin!, "html", "syntaxes", "html.tmLanguage.json"),
    "text.html.derivative": path.join(builtin!, "html", "syntaxes", "html-derivative.tmLanguage.json"),
  };
  const registry = new textmate.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (patterns: string[]) => new oniguruma.OnigScanner(patterns),
      createOnigString: (text: string) => new oniguruma.OnigString(text),
    }),
    loadGrammar: async (scope: string) =>
      files[scope] ? textmate.parseRawGrammar(fs.readFileSync(files[scope], "utf8"), files[scope]) : null,
  });
  const grammar = await registry.loadGrammar("text.html.php.blade");
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

test("an echo in the text of a template is PHP", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize("<p>{{ $post->title }}</p>");
  const variable = scopesOf(lines, "post");
  assert.ok(has(variable, "source.php") && has(variable, "variable.other.php"), variable.join(" "));
  assert.ok(has(scopesOf(lines, "{{"), "punctuation.section.embedded.begin.blade"));
});

test("an echo inside an attribute value is PHP, not attribute text", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`<a href="{{ route('posts.show', $post) }}" class="link">x</a>`);
  const variable = scopesOf(lines, "post");
  assert.ok(has(variable, "source.php") && has(variable, "variable.other.php"), variable.join(" "));
  assert.ok(has(variable, "meta.tag"), "still inside the tag");
  assert.ok(has(scopesOf(lines, "route"), "source.php"));
  // The rest of the tag is HTML as before.
  assert.ok(has(scopesOf(lines, "class"), "entity.other.attribute-name"));
  assert.ok(has(scopesOf(lines, "link"), "string.quoted.double.html"));
});

test("a raw echo and a comment inside a tag", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`<div {!! $attributes !!} {{-- note --}} id="a">`);
  assert.ok(has(scopesOf(lines, "attributes"), "variable.other.php"));
  assert.ok(has(scopesOf(lines, " note "), "comment.block.blade"));
  assert.ok(has(scopesOf(lines, "id"), "entity.other.attribute-name"));
});

test("a directive inside a tag, with PHP arguments", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`<li @class(['active' => $isActive]) @disabled($off)>`);
  assert.ok(has(scopesOf(lines, "@class"), "keyword.control.directive.blade"));
  assert.ok(has(scopesOf(lines, "isActive"), "variable.other.php"));
  assert.ok(has(scopesOf(lines, "@disabled"), "keyword.control.directive.blade"));
});

test("a component's bound attribute is PHP; a plain one and an escaped one are not", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`<x-alert :type="$type" title="Hello" ::class="{ open }" />`);
  assert.ok(has(scopesOf(lines, ":type"), "entity.other.attribute-name.bound.blade"));
  const bound = scopesOf(lines, "type");
  assert.ok(has(bound, "source.php") && has(bound, "variable.other.php"), bound.join(" "));
  assert.ok(!has(scopesOf(lines, "Hello"), "source.php"));
  assert.ok(!lines.flat().some((t) => t.text.includes("open") && has(t.scopes, "source.php")), "::class is Alpine's");
});

test("an e-mail address in an attribute is not a directive", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(`<a href="mailto:me@example.com">me@example.com</a>`);
  assert.ok(!lines.flat().some((t) => has(t.scopes, "keyword.control.directive.blade")));
});

test("in a script an echo is PHP, and `@` words are left to JavaScript", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(["<script>", "  const id = {{ $post->id }};", "  @media", "</script>"].join("\n"));
  assert.ok(has(scopesOf(lines, "post"), "variable.other.php"));
  assert.ok(!lines.flat().some((t) => has(t.scopes, "keyword.control.directive.blade")));
});

test("directives and their PHP in the text of a template, as before", { skip }, async () => {
  const tokenize = await tokenizer();
  const lines = tokenize(["@foreach ($posts as $post)", "  @continue", "@endforeach", "@{{ raw }}"].join("\n"));
  assert.ok(has(scopesOf(lines, "@foreach"), "keyword.control.directive.blade"));
  assert.ok(has(scopesOf(lines, "posts"), "variable.other.php"));
  assert.ok(has(scopesOf(lines, "@endforeach"), "keyword.control.directive.blade"));
  assert.ok(has(scopesOf(lines, "@{{"), "constant.character.escape.blade"));
});
