// The scenes of the README's pictures, run inside VS Code by run.js. Each drives the
// editor as a user would (commands and typed characters) and asks run.js for pictures.

const path = require("node:path");
const vscode = require("vscode");

const project = process.env.PHOXITE_SHOTS_PROJECT;
const service = process.env.PHOXITE_SHOTS_URL;
const wanted = (process.env.PHOXITE_SHOTS_SCENES || "").split(",").filter(Boolean);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const run = (command, ...args) => vscode.commands.executeCommand(command, ...args);

async function ask(pathname, params) {
  const response = await fetch(`${service}${pathname}?${new URLSearchParams(params)}`);
  if (!response.ok) throw new Error(await response.text());
}

/** A still picture, `images/<name>.png`. */
const shot = (name) => ask("/shot", { name });
/** A frame of animation `name`, shown for `delay` ms. */
const frame = (name, delay = 90) => ask("/frame", { name, delay });

async function until(label, f, done, timeout = 90_000) {
  const deadline = Date.now() + timeout;
  let last;
  while (Date.now() < deadline) {
    last = await f();
    if (done(last)) return last;
    await sleep(300);
  }
  throw new Error(`${label}: gave up`);
}

async function open(rel) {
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(path.join(project, rel)));
  return vscode.window.showTextDocument(doc, { preview: false });
}

/** Puts the caret at the end of the line holding `needle`, that line near the top third. */
function caretAfter(editor, needle, top = 6) {
  const index = editor.document.getText().indexOf(needle);
  if (index < 0) throw new Error(`${needle} not in ${editor.document.uri.fsPath}`);
  const line = editor.document.positionAt(index).line;
  const end = editor.document.lineAt(line).range.end;
  editor.selection = new vscode.Selection(end, end);
  const first = Math.max(0, line - top);
  editor.revealRange(new vscode.Range(first, 0, first, 0), vscode.TextEditorRevealType.AtTop);
  return end;
}

/** Types `text` a character at a time, a frame after each. */
async function type(name, text, delay = 70) {
  for (const character of text) {
    await run("type", { text: character });
    await sleep(60);
    await frame(name, delay);
  }
}

/** With PHOXITE_SHOTS_DEBUG, the moments an animation holds are also kept as pictures. */
let held = 0;
async function hold(name, delay) {
  await frame(name, delay);
  if (process.env.PHOXITE_SHOTS_DEBUG) await shot(`_${name}-${++held}`);
}

/** Shows the suggestions at the caret, and holds the picture for `delay` ms. */
async function suggest(name, delay = 1100) {
  await run("editor.action.triggerSuggest");
  await sleep(900);
  await hold(name, delay);
}

async function accept(name, delay = 500) {
  await run("acceptSelectedSuggestion");
  await sleep(250);
  await hold(name, delay);
}

async function closeAll() {
  await run("workbench.action.closeAllEditors");
  await run("workbench.action.closePanel");
  await run("notifications.clearAll");
}

const laravel = () => vscode.extensions.getExtension("phoxite.phoxite").exports.laravel;

const scenes = {
  // Completion that knows the model: its columns and relations, then the related model's.
  async completion() {
    const name = "completion";
    await run("workbench.action.closeSidebar");
    const editor = await open("app/Http/Controllers/PostController.php");
    caretAfter(editor, "$published = $post->published_at;", 9);
    await sleep(1500);
    await frame(name, 700);
    await run("editor.action.insertLineAfter");
    await type(name, "$email = $post->");
    await suggest(name, 1500);
    await type(name, "author", 130);
    await sleep(400);
    await frame(name, 700);
    await run("hideSuggestWidget");
    await type(name, "->");
    await suggest(name, 1500);
    await type(name, "em", 160);
    await sleep(500);
    await frame(name, 800);
    await accept(name);
    await type(name, ";");
    await frame(name, 2200);
    await run("workbench.action.files.revert");
  },

  // The strings that name routes and views complete, and go to what they name.
  async laravel() {
    const name = "laravel";
    await run("workbench.action.closeSidebar");
    const editor = await open("app/Http/Controllers/PostController.php");
    caretAfter(editor, "$post = $request->user()->posts()->create", 9);
    await sleep(1200);
    await frame(name, 700);
    await run("editor.action.insertLineAfter");
    await type(name, "$url = route('");
    await suggest(name, 1400);
    await type(name, "posts.s", 140);
    await sleep(500);
    await frame(name, 900);
    await accept(name, 900);
    await run("cursorLineEnd");
    await type(name, ";");
    await run("editor.action.insertLineAfter");
    await type(name, "$page = view('");
    await suggest(name, 1400);
    await type(name, "posts.i", 140);
    await sleep(500);
    await frame(name, 900);
    await accept(name, 600);
    await run("cursorLineEnd");
    await type(name, ";");
    await frame(name, 2200);
    await run("workbench.action.files.revert");
  },

  // The routes view beside a route file with its hints.
  async routes() {
    await open("routes/web.php");
    await run("workbench.view.explorer");
    await run("phoxite.laravel.routes.focus");
    await sleep(1500);
    // The view gets the side bar: the file tree above it is folded away.
    await ask("/collapse", { pane: "blog" });
    await sleep(1500);
    await shot("routes");
  },

  async modelView() {
    await run("workbench.action.closeSidebar");
    await run("phoxite.server.laravel.modelView", "App\\Models\\Post");
    await sleep(2500);
    await shot("model-view");
  },

  async changeSignature() {
    await run("workbench.action.closeSidebar");
    const editor = await open("app/Policies/PostPolicy.php");
    const text = editor.document.getText();
    const at = editor.document.positionAt(text.indexOf("function update") + "function ".length + 1);
    editor.selection = new vscode.Selection(at, at);
    await sleep(1500);
    await run("phoxite.refactor.changeSignature");
    await sleep(2500);
    await shot("change-signature");
  },

  // Blade inside tags, attributes and components.
  async blade() {
    await run("workbench.action.closeSidebar");
    await open("resources/views/posts/show.blade.php");
    await sleep(2500);
    await shot("blade");
  },
};

exports.run = async function () {
  await vscode.extensions.getExtension("phoxite.phoxite").activate();
  await until("the Laravel project", async () => laravel().isActive, (active) => active === true);
  await until("routes", () => laravel().routes().catch(() => []), (found) => found.length > 0);
  await sleep(2000);
  for (const [name, scene] of Object.entries(scenes)) {
    if (wanted.length && !wanted.includes(name)) continue;
    await closeAll();
    await sleep(400);
    try {
      await scene();
      console.log(`scene ${name}: done`);
    } catch (e) {
      console.log(`scene ${name}: FAILED ${e.message}`);
    }
  }
};
