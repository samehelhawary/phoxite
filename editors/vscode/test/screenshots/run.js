// Takes the pictures of the README (`images/`): runs VS Code on a copy of the Laravel
// fixture with only this extension, plays the scenes of scenes.js in it, and captures
// the window through the Chrome DevTools Protocol. A scene asks for a picture, or for
// a frame of an animation, over HTTP; the animations are written as GIFs at the end.
//
//   node test/screenshots/run.js [scene…]
//
// Needs a server: server/phoxite-lsp(.exe), or PHOXITE_SERVER pointing at one.

const fs = require("node:fs");
const http = require("node:http");
const os = require("node:os");
const path = require("node:path");
const { runTests } = require("@vscode/test-electron");
const { PNG } = require("pngjs");

const WIDTH = 1180;
const HEIGHT = 720;
const DEBUG_PORT = 9339;
const extension = path.resolve(__dirname, "../..");
const out = path.join(extension, "images");

function copyDir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const entry of fs.readdirSync(from, { withFileTypes: true })) {
    const src = path.join(from, entry.name);
    const dst = path.join(to, entry.name);
    if (entry.isDirectory()) copyDir(src, dst);
    else fs.copyFileSync(src, dst);
  }
}

/** The DevTools connection to the workbench window. */
class Window {
  static async connect() {
    let target;
    for (let attempt = 0; attempt < 50 && !target; attempt++) {
      const targets = await fetch(`http://127.0.0.1:${DEBUG_PORT}/json/list`).then((r) => r.json()).catch(() => []);
      target = targets.find((t) => t.type === "page" && /workbench/.test(t.url));
      if (!target) await new Promise((r) => setTimeout(r, 200));
    }
    if (!target) throw new Error("no workbench window on the debugging port");
    const window = new Window(new WebSocket(target.webSocketDebuggerUrl));
    await new Promise((resolve, reject) => {
      window.socket.addEventListener("open", resolve);
      window.socket.addEventListener("error", reject);
    });
    await window.send("Emulation.setDeviceMetricsOverride", { width: WIDTH, height: HEIGHT, deviceScaleFactor: 1, mobile: false });
    return window;
  }

  constructor(socket) {
    this.socket = socket;
    this.next = 0;
    this.waiting = new Map();
    socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      const done = this.waiting.get(message.id);
      if (!done) return;
      this.waiting.delete(message.id);
      if (message.error) done.reject(new Error(message.error.message));
      else done.resolve(message.result);
    });
  }

  send(method, params = {}) {
    const id = ++this.next;
    return new Promise((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async png() {
    const shot = await this.send("Page.captureScreenshot", { format: "png" });
    return Buffer.from(shot.data, "base64");
  }
}

/** Writes `frames` (`{ png, delay }`) as an animation that plays forever. */
async function writeGif(file, frames) {
  const { GIFEncoder, quantize, applyPalette } = require("gifenc");
  const gif = GIFEncoder();
  // A frame that shows what the one before does only lengthens it.
  const kept = [];
  for (const frame of frames) {
    const last = kept[kept.length - 1];
    if (last && last.png.equals(frame.png)) last.delay += frame.delay;
    else kept.push({ ...frame });
  }
  for (const frame of kept) {
    const { data, width, height } = PNG.sync.read(frame.png);
    const palette = quantize(data, 256);
    gif.writeFrame(applyPalette(data, palette), width, height, { palette, delay: frame.delay });
  }
  gif.finish();
  fs.writeFileSync(file, gif.bytes());
  return kept.length;
}

async function main() {
  const fixtures = path.resolve(extension, "../../fixtures");
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "phoxite-shots-"));
  // The folder's name is in the window's title and the Explorer.
  const project = path.join(tmp, "blog");
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
      "security.workspace.trust.enabled": false,
      "workbench.startupEditor": "none",
      "workbench.secondarySideBar.defaultVisibility": "hidden",
      "workbench.layoutControl.enabled": false,
      "workbench.tips.enabled": false,
      "workbench.colorTheme": "Default Dark Modern",
      "window.commandCenter": false,
      // A native title bar is not part of the page: the pictures start at the editor,
      // without "[Extension Development Host]".
      "window.titleBarStyle": "native",
      "window.menuBarVisibility": "hidden",
      // The suggestions are the subject; the parameter hints would cover them.
      "editor.parameterHints.enabled": false,
      "chat.commandCenter.enabled": false,
      "chat.disableAIFeatures": true,
      "editor.minimap.enabled": false,
      "editor.fontSize": 14,
      "editor.stickyScroll.enabled": false,
      "breadcrumbs.enabled": false,
      "git.enabled": false,
      "telemetry.telemetryLevel": "off",
      "update.mode": "none",
      "extensions.ignoreRecommendations": true,
    }),
  );
  fs.mkdirSync(out, { recursive: true });

  let window;
  const animations = new Map();
  const taken = [];
  const service = http.createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://localhost");
      window ??= await Window.connect();
      if (url.pathname === "/collapse") {
        // Collapses the pane of the side bar whose header begins with `pane`: what a
        // click on that header does, and no command does for the file tree.
        const pane = JSON.stringify(url.searchParams.get("pane").toLowerCase());
        const expression = `(() => {
          const header = [...document.querySelectorAll(".pane-header.expanded")]
            .find((h) => h.textContent.trim().toLowerCase().startsWith(${pane}));
          header?.click();
          return !!header;
        })()`;
        await window.send("Runtime.evaluate", { expression });
        response.end("ok");
        return;
      }
      const png = await window.png();
      if (url.pathname === "/shot") {
        const name = `${url.searchParams.get("name")}.png`;
        fs.writeFileSync(path.join(out, name), png);
        taken.push(name);
      } else if (url.pathname === "/frame") {
        const name = url.searchParams.get("name");
        const frames = animations.get(name) ?? [];
        frames.push({ png, delay: Number(url.searchParams.get("delay")) || 100 });
        animations.set(name, frames);
      }
      response.end("ok");
    } catch (e) {
      response.statusCode = 500;
      response.end(String(e));
    }
  });
  await new Promise((resolve) => service.listen(0, "127.0.0.1", resolve));

  const options = {
    extensionDevelopmentPath: extension,
    extensionTestsPath: path.join(__dirname, "scenes.js"),
    launchArgs: [
      project,
      "--disable-extensions",
      `--user-data-dir=${userData}`,
      `--remote-debugging-port=${DEBUG_PORT}`,
      "--skip-welcome",
      "--skip-release-notes",
    ],
    extensionTestsEnv: {
      PHOXITE_SHOTS_PROJECT: project,
      PHOXITE_SHOTS_URL: `http://127.0.0.1:${service.address().port}`,
      PHOXITE_SHOTS_SCENES: process.argv.slice(2).join(","),
    },
  };
  try {
    await runTests(options);
  } catch (e) {
    console.error("the scenes failed", e);
    process.exitCode = 1;
  }
  service.close();
  for (const [name, frames] of animations) {
    const file = path.join(out, `${name}.gif`);
    const count = await writeGif(file, frames);
    taken.push(`${name}.gif (${count} frames, ${(fs.statSync(file).size / 1024).toFixed(0)} KB)`);
  }
  console.log(`\n${out}:\n  ${taken.join("\n  ")}`);
}

void main();
