// Runs the end-to-end suite (suite.js) inside VS Code on a copy of the Laravel
// fixture (and once on each of the WordPress, Drupal, CodeIgniter, Yii and Symfony ones), with only this
// extension loaded.
//
//   node test/e2e/run.js [path to Code executable]
//
// PHOXITE_E2E_ONLY=wordpress (or one-project, two-projects, drupal, codeigniter, yii, symfony) runs that
// part alone.
// Without a path it downloads a VS Code build. The extension needs a server:
// server/phoxite-lsp(.exe), or PHOXITE_SERVER pointing at one.

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { runTests } = require("@vscode/test-electron");

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const src = path.join(from, entry.name);
    const dst = path.join(to, entry.name);
    if (entry.isDirectory()) copyDir(src, dst);
    else fs.copyFileSync(src, dst);
  }
}

/**
 * Runs the suite in a fresh VS Code on a fresh copy of the fixture. In mode `two-projects`
 * the workspace has two folders, each a copy of the fixture, and the suite runs the
 * checks about that instead. In modes `wordpress`, `drupal`, `codeigniter`, `yii` and `symfony` the project is that fixture.
 */
async function run(mode) {
  const extension = path.resolve(__dirname, "../..");
  const fixtures = path.resolve(extension, "../../fixtures");
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "phoxite-e2e-"));
  const project = path.join(tmp, "app");
  if (mode === "wordpress" || mode === "drupal" || mode === "codeigniter" || mode === "yii" || mode === "symfony") {
    copyDir(path.join(fixtures, `${mode}-app`), project);
  } else {
    copyDir(path.join(fixtures, "laravel-app"), project);
    copyDir(path.join(fixtures, "laravel-stubs"), path.join(project, "vendor"));
  }
  const userData = path.join(tmp, "user-data");
  const settings = path.join(userData, "User", "settings.json");
  fs.mkdirSync(path.dirname(settings), { recursive: true });
  // A relative path is from where the suite is started, not from the project VS Code opens.
  let server = process.env.PHOXITE_SERVER && path.resolve(process.env.PHOXITE_SERVER);
  if (server && process.platform === "win32" && !fs.existsSync(server) && fs.existsSync(server + ".exe")) {
    server += ".exe";
  }
  fs.writeFileSync(
    settings,
    JSON.stringify({
      ...(server ? { "phoxite.server.path": server } : {}),
      "phoxite.warnAboutOtherServers": false,
      "workbench.startupEditor": "none",
      "security.workspace.trust.enabled": false,
    }),
  );
  let opened = project;
  const second = path.join(tmp, "shop");
  if (mode === "two-projects") {
    copyDir(project, second);
    opened = path.join(tmp, "two.code-workspace");
    fs.writeFileSync(opened, JSON.stringify({ folders: [{ path: "app" }, { path: "shop" }] }));
  }
  const options = {
    extensionDevelopmentPath: extension,
    extensionTestsPath: path.join(__dirname, "suite.js"),
    launchArgs: [opened, "--disable-extensions", `--user-data-dir=${userData}`, "--skip-welcome", "--skip-release-notes"],
    extensionTestsEnv: { PHOXITE_E2E_PROJECT: project, PHOXITE_E2E_SECOND: second, PHOXITE_E2E_MODE: mode },
  };
  if (process.argv[2]) options.vscodeExecutablePath = process.argv[2];
  console.log(`\n== ${mode} ==`);
  await runTests(options);
}

async function main() {
  try {
    const only = process.env.PHOXITE_E2E_ONLY;
    for (const mode of ["one-project", "two-projects", "wordpress", "drupal", "codeigniter", "yii", "symfony"]) {
      if (!only || only === mode) await run(mode);
    }
  } catch (e) {
    console.error("the end-to-end tests failed", e);
    process.exitCode = 1;
  }
}

void main();
