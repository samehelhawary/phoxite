// ESLint for the extension's sources (TypeScript), its tests and the scripts of the
// webview pages (plain browser JavaScript). Formatting is not its business.

import js from "@eslint/js";
import tseslint from "typescript-eslint";

const browser = {
  acquireVsCodeApi: "readonly",
  document: "readonly",
  window: "readonly",
};

const node = {
  __dirname: "readonly",
  Buffer: "readonly",
  console: "readonly",
  exports: "writable",
  module: "writable",
  process: "readonly",
  require: "readonly",
  setTimeout: "readonly",
  clearTimeout: "readonly",
  fetch: "readonly",
  URL: "readonly",
  URLSearchParams: "readonly",
  WebSocket: "readonly",
};

export default tseslint.config(
  { ignores: ["dist/**", "server/**", "node_modules/**", ".vscode-test/**"] },
  js.configs.recommended,
  {
    files: ["src/**/*.ts", "test/**/*.mts"],
    extends: [...tseslint.configs.recommended],
    rules: {
      // `_` is how an argument that has to be there says it isn't used.
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
      eqeqeq: ["error", "always"],
      "no-throw-literal": "error",
    },
  },
  { files: ["media/**/*.js"], languageOptions: { sourceType: "script", globals: browser }, rules: { eqeqeq: ["error", "always"] } },
  { files: ["test/e2e/**/*.js", "test/screenshots/**/*.js"], languageOptions: { sourceType: "commonjs", globals: node } },
  { files: ["eslint.config.mjs"], languageOptions: { sourceType: "module" } },
);
