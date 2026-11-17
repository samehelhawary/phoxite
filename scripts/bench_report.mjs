// Runs the budget and memory tests and writes what they measured into README.md and
// docs/engine/performance.md, between their `<!-- bench:… -->` markers.
//
//   node scripts/bench_report.mjs           run both tests (release builds), then write
//   node scripts/bench_report.mjs --no-run  write from the reports of an earlier run
//
// Needs the laravel/framework corpus (scripts/php-corpus.sh laravel-framework) and
// Intelephense (npm i -g intelephense) for the memory test. The tests leave their reports in
// target/php-budgets.json and target/php-memory.json.

import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const file = (name) => path.join(root, name);

/** `text` with what is between the `name` markers replaced by `body`. */
export function withBlock(text, name, body) {
  const start = `<!-- bench:${name}:start -->`;
  const end = `<!-- bench:${name}:end -->`;
  const from = text.indexOf(start);
  const to = text.indexOf(end);
  if (from < 0 || to < from) throw new Error(`no ${start} … ${end} block`);
  return `${text.slice(0, from + start.length)}\n${body.trim()}\n${text.slice(to)}`;
}

/** `215.44` → `215 ms`, `2.3` → `2.3 ms`, `1504.62` → `1.5 s`. */
export function duration(ms) {
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)} s`;
  if (ms >= 100) return `${Math.round(ms)} ms`;
  if (ms >= 10) return `${ms.toFixed(0)} ms`;
  return `${ms.toFixed(1)} ms`;
}

const OS_NAMES = { windows: "Windows", linux: "Linux", macos: "macOS" };

/** `2026-11-17, Windows x86_64, 16 threads`. */
export function machine(report) {
  const date = new Date(report.unix_time * 1000).toISOString().slice(0, 10);
  return `${date}, ${OS_NAMES[report.os] ?? report.os} ${report.arch}, ${report.threads} threads`;
}

const table = (rows) => rows.map((cells) => `| ${cells.join(" | ")} |`).join("\n");

/** The README's block: the comparison with Intelephense, then the latencies. */
export function readme(budgets, memory, corpus) {
  const laravel = Object.fromEntries(
    budgets.results.filter((r) => r.workload === "php_laravel").map((r) => [r.measure, r.median_ms]),
  );
  const files = memory.files.toLocaleString("en-US");
  const intelephense = `Intelephense ${memory.intelephense.version}`;
  return [
    `On ${corpus} (${files} PHP files), release build, ${machine(memory)}:`,
    "",
    table([
      ["", "Phoxite", intelephense],
      ["---", "---", "---"],
      ["Index from an empty cache", `${memory.engine.index_s} s`, `${memory.intelephense.index_s} s`],
      ["Memory once indexed", `${memory.engine.ready_mb} MB`, `${memory.intelephense.after_indexing_mb} MB`],
      [
        "Memory after hover, completion and diagnostics in a 2,500-line file",
        `${memory.engine.after_requests_mb} MB`,
        "not measured",
      ],
    ]),
    "",
    table([
      ["Phoxite, same project", ""],
      ["---", "---"],
      ["Start on the index of an earlier run", duration(laravel.warm_start)],
      ["Hover right after a keystroke", duration(laravel.hover_after_keystroke)],
      ["Completion right after a keystroke", duration(laravel.completion_after_keystroke)],
      ["Diagnostics after an edit (120 ms of it is the debounce)", duration(laravel.diagnostics_after_edit)],
    ]),
  ].join("\n");
}

const WORKLOADS = { php_laravel: "laravel/framework", php_fixture: "fixtures/php-library" };

/** performance.md's budget table. */
export function budgetTable(budgets) {
  const rows = budgets.results.map((r) => [
    `\`${r.workload}/${r.measure}\``,
    duration(r.budget_ms),
    duration(r.median_ms),
  ]);
  const projects = [...new Set(budgets.results.map((r) => WORKLOADS[r.workload] ?? r.workload))].join(" and ");
  return [
    table([["Measure", "Budget", "Median"], ["---", "---", "---"], ...rows]),
    "",
    `Medians of \`tests/budgets.rs\` on ${projects}, release build, ${machine(budgets)}.`,
  ].join("\n");
}

/** performance.md's memory table. */
export function memoryTable(memory, corpus) {
  const peak = (mb) => (mb == null ? "" : `, peak ${mb} MB`);
  const e = memory.engine;
  const i = memory.intelephense;
  return [
    table([
      ["", "Index time", "Working set"],
      ["---", "---", "---"],
      [
        "Phoxite",
        `${e.index_s} s`,
        `${e.ready_mb} MB when ready; ${e.after_requests_mb} MB after hover, completion and the inspections on \`Builder.php\` (the bundled stubs load on first use)${peak(e.peak_mb)}`,
      ],
      ["Intelephense " + i.version, `${i.index_s} s`, `${i.after_indexing_mb} MB after \`indexingEnded\`${peak(i.peak_mb)}`],
    ]),
    "",
    `${corpus}, ${memory.files.toLocaleString("en-US")} PHP files, ${machine(memory)}.`,
  ].join("\n");
}

function run(test) {
  const args = ["test", "-p", "phoxite", "--release", "--test", test, "--", "--ignored", "--nocapture"];
  console.log(`cargo ${args.join(" ")}`);
  const env = { ...process.env };
  delete env.PHOXITE_BUDGET_FACTOR;
  const done = spawnSync("cargo", args, { cwd: root, stdio: "inherit", env });
  if (done.status !== 0) throw new Error(`the ${test} test failed`);
}

/** `laravel/framework v13.40.0`, from the corpus checkout. */
function corpusName() {
  const dir = path.resolve(root, process.env.PHP_CORPUS_DIR ?? "../php-corpus", "laravel-framework");
  const tag = spawnSync("git", ["-C", dir, "describe", "--tags", "--exact-match"], { encoding: "utf8" });
  if (tag.status !== 0) throw new Error(`no laravel/framework corpus at ${dir} (scripts/php-corpus.sh)`);
  return `laravel/framework ${tag.stdout.trim()}`;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const corpus = corpusName();
  if (!process.argv.includes("--no-run")) {
    run("budgets");
    run("memory");
  }
  const read = (name) => JSON.parse(fs.readFileSync(file(`target/${name}`), "utf8"));
  const budgets = read("php-budgets.json");
  const memory = read("php-memory.json");
  for (const [name, report] of [["budgets", budgets], ["memory", memory]]) {
    if (!report.release) throw new Error(`the ${name} report is of a debug build`);
  }
  if (budgets.budget_factor !== 1) throw new Error("the budgets report was made with PHOXITE_BUDGET_FACTOR set");
  if (!budgets.results.some((r) => r.workload === "php_laravel")) {
    throw new Error("the budgets report has no laravel/framework run");
  }

  const write = (name, blocks) => {
    let text = fs.readFileSync(file(name), "utf8");
    for (const [block, body] of blocks) text = withBlock(text, block, body);
    fs.writeFileSync(file(name), text);
    console.log(`wrote ${name}`);
  };
  write("README.md", [["summary", readme(budgets, memory, corpus)]]);
  write("docs/engine/performance.md", [
    ["budgets", budgetTable(budgets)],
    ["memory", memoryTable(memory, corpus)],
  ]);
}
