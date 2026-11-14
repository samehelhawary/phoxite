// Runs the end-to-end suite (suite.js) inside VS Code on a copy of the Laravel
// fixture, with only this extension loaded.
//
//   node test/e2e/run.js [path to Code executable]
//
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

async function main() {
  const extension = path.resolve(__dirname, "../..");
  const fixtures = path.resolve(extension, "../../fixtures");
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "phoxite-e2e-"));
  const project = path.join(tmp, "app");
  copyDir(path.join(fixtures, "laravel-app"), project);
  copyDir(path.join(fixtures, "laravel-stubs"), path.join(project, "vendor"));
  const userData = path.join(tmp, "user-data");
  const settings = path.join(userData, "User", "settings.json");
  fs.mkdirSync(path.dirname(settings), { recursive: true });
  const server = process.env.PHOXITE_SERVER;
  fs.writeFileSync(
    settings,
    JSON.stringify({
      ...(server ? { "phoxite.server.path": server } : {}),
      "phoxite.warnAboutOtherServers": false,
      "workbench.startupEditor": "none",
      "security.workspace.trust.enabled": false,
    }),
  );
  const options = {
    extensionDevelopmentPath: extension,
    extensionTestsPath: path.join(__dirname, "suite.js"),
    launchArgs: [project, "--disable-extensions", `--user-data-dir=${userData}`, "--skip-welcome", "--skip-release-notes"],
    extensionTestsEnv: { PHOXITE_E2E_PROJECT: project },
  };
  if (process.argv[2]) options.vscodeExecutablePath = process.argv[2];
  try {
    await runTests(options);
  } catch (e) {
    console.error("the end-to-end tests failed", e);
    process.exitCode = 1;
  }
}

void main();
