// Writes docs/settings.md, the reference of `phoxite.json`, from its schema
// (editors/vscode/schemas/phoxite.schema.json), which a test keeps in step with the
// extension's settings.
//   node scripts/settings_doc.mjs           write it
//   node scripts/settings_doc.mjs --check   fail when it is out of date
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.join(path.dirname(fileURLToPath(import.meta.url)), "..");
const schema = JSON.parse(fs.readFileSync(path.join(root, "editors/vscode/schemas/phoxite.schema.json"), "utf8"));
const out = path.join(root, "docs/settings.md");

const resolve = (node) => (node.$ref ? schema.definitions[node.$ref.split("/").pop()] : node);

function typeOf(node) {
  if (node.enum) return node.enum.map((value) => `\`${value}\``).join(", ");
  const type = Array.isArray(node.type) ? node.type.join(" or ") : node.type;
  if (type === "array" && node.items) return `array of ${resolve(node.items).type ?? "values"}`;
  return type ?? "";
}

// A table cell: one line, no bar that would end it.
const cell = (text) => String(text).replace(/\s*\n\s*/g, " ").replace(/\|/g, "\\|");

const rows = [];
function walk(node, prefix) {
  for (const [name, raw] of Object.entries(node.properties ?? {})) {
    if (name === "$schema") continue;
    const setting = resolve(raw);
    const key = prefix + name;
    const group = setting.type === "object" && setting.properties;
    const text = setting.markdownDescription ?? setting.description ?? raw.description ?? "";
    if (!group || text) {
      const fallback = setting.default === undefined || group ? "" : `\`${JSON.stringify(setting.default)}\``;
      rows.push(`| \`${key}\` | ${group ? "" : cell(typeOf(setting))} | ${cell(fallback)} | ${cell(text)} |`);
    }
    if (group) walk(setting, key + ".");
  }
}
walk(schema, "");

const text = `# Settings

${cell(schema.description)} The file is \`phoxite.json\`.

In VS Code the same settings are under \`phoxite.\` (\`phoxite.format.lineWidth\`); another
editor sends them as \`docs/editors.md\` says. This page is written from the file's schema
(\`node scripts/settings_doc.mjs\`): don't edit the table by hand. The ids \`inspections\` takes
are in [inspections.md](inspections.md).

\`\`\`json
{
  "$schema": "${schema.$id}",
  "phpVersion": "8.3",
  "frameworks": { "symfony": "disabled" }
}
\`\`\`

| Setting | Type | Default | What |
| --- | --- | --- | --- |
${rows.join("\n")}
`;

if (process.argv.includes("--check")) {
  const written = fs.existsSync(out) ? fs.readFileSync(out, "utf8").replace(/\r\n/g, "\n") : "";
  if (written !== text) {
    console.error("docs/settings.md is out of date: run `node scripts/settings_doc.mjs`");
    process.exit(1);
  }
} else {
  fs.writeFileSync(out, text);
  console.log(`wrote ${path.relative(root, out)}`);
}
