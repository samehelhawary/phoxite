// The New Eloquent Model form: rows of fields, relations and indexes, and what to
// generate with the model. Create posts the form to the extension, which checks it
// and has the server write the files.
(function () {
  const vscode = acquireVsCodeApi();
  const data = JSON.parse(document.getElementById("data").textContent);
  const form = document.getElementById("new-model");
  // What was filled in before the window reloaded.
  const saved = vscode.getState();
  const start = (saved && saved.form) || data.form;
  const error = document.getElementById("error");
  const lists = {
    fields: document.getElementById("fields"),
    relations: document.getElementById("relations"),
    indexes: document.getElementById("indexes"),
  };

  function element(tag, props, children) {
    const node = Object.assign(document.createElement(tag), props || {});
    for (const child of children || []) node.append(child);
    return node;
  }

  function input(name, placeholder, value, className) {
    const node = element("input", { type: "text", name, placeholder, value: value || "", className: className || "" });
    node.setAttribute("aria-label", placeholder);
    return node;
  }

  function select(name, label, options, value) {
    const node = element(
      "select",
      { name },
      options.map((o) => element("option", { value: o.value, textContent: o.label })),
    );
    node.value = value;
    node.setAttribute("aria-label", label);
    return node;
  }

  function check(name, label, checked) {
    return element("label", { className: "check" }, [element("input", { type: "checkbox", name, checked: !!checked }), label]);
  }

  function remove(title) {
    const node = element("button", { type: "button", className: "remove", textContent: "✕", title });
    node.dataset.action = "remove";
    node.setAttribute("aria-label", title);
    return node;
  }

  const plain = (values) => values.map((value) => ({ value, label: value }));
  const shortName = (fqn) => fqn.slice(fqn.lastIndexOf("\\") + 1);

  function addField(field) {
    const f = field || { name: "", ty: "string", default: "" };
    const row = element("div", { className: "row" }, [
      input("name", "Column name", f.name),
      select("ty", "Column type", plain(data.columnTypes), f.ty),
      input("default", "Default", f.default, "narrow"),
      check("nullable", "Nullable", f.nullable),
      check("unique", "Unique", f.unique),
      check("index", "Index", f.index),
      remove("Remove the field"),
    ]);
    lists.fields.append(row);
    return row;
  }

  function addRelation(relation) {
    const r = relation || { kind: "belongsTo", model: "", name: "" };
    const models = [{ value: "", label: "Related model…" }].concat(
      data.models.map((m) => ({ value: m, label: shortName(m) + "  (" + m + ")" })),
    );
    const kind = select("kind", "Relation", plain(data.relationKinds), r.kind);
    const model = select("model", "Related model", models, r.model);
    const name = input("name", "Name (optional)", r.name);
    // A morphTo relation has no model, and needs its name.
    const sync = () => {
      const morph = kind.value === "morphTo";
      model.classList.toggle("hidden", morph);
      name.placeholder = morph ? "Name, e.g. commentable" : "Name (optional)";
    };
    kind.addEventListener("change", sync);
    sync();
    const row = element("div", { className: "row" }, [kind, model, name, remove("Remove the relation")]);
    lists.relations.append(row);
    return row;
  }

  function addIndex(index) {
    const i = index || { columns: "", unique: false };
    const row = element("div", { className: "row" }, [
      input("columns", "Columns, e.g. status, published_at", i.columns),
      check("unique", "Unique", i.unique),
      remove("Remove the index"),
    ]);
    lists.indexes.append(row);
    return row;
  }

  // The head: the class name, and the module when the application has modules.
  const head = document.getElementById("head");
  const name = input("modelName", "Class name, e.g. Post", start.name);
  head.append(name);
  let moduleSelect = null;
  if (data.modules.length) {
    const modules = [{ value: "", label: "Application  (" + data.namespace + ")" }].concat(
      data.modules.map((m) => ({ value: m, label: m + "  (module)" })),
    );
    moduleSelect = select("module", "Module", modules, start.module);
    head.append(moduleSelect);
  }

  const GENERATE = [
    ["timestamps", "Timestamps"],
    ["softDeletes", "Soft deletes"],
    ["migration", "Migration"],
    ["factory", "Factory"],
    ["seeder", "Seeder"],
    ["formRequests", "Form Requests"],
    ["resource", "JSON resource"],
    ["policy", "Policy"],
  ];
  const generate = document.getElementById("generate");
  for (const [key, label] of GENERATE) generate.append(check(key, label, start[key]));
  const controller = select("controller", "Controller", data.controllerKinds, start.controller);
  generate.append(controller);

  start.fields.forEach(addField);
  start.relations.forEach(addRelation);
  start.indexes.forEach(addIndex);

  const value = (row, field) => row.querySelector("[name=" + field + "]").value;
  const checked = (row, field) => row.querySelector("[name=" + field + "]").checked;

  function read() {
    const out = {
      name: name.value,
      module: moduleSelect ? moduleSelect.value : start.module,
      fields: [...lists.fields.children].map((row) => ({
        name: value(row, "name"),
        ty: value(row, "ty"),
        default: value(row, "default"),
        nullable: checked(row, "nullable"),
        unique: checked(row, "unique"),
        index: checked(row, "index"),
      })),
      relations: [...lists.relations.children].map((row) => ({
        kind: value(row, "kind"),
        model: value(row, "model"),
        name: value(row, "name"),
      })),
      indexes: [...lists.indexes.children].map((row) => ({ columns: value(row, "columns"), unique: checked(row, "unique") })),
      controller: controller.value,
    };
    for (const [key] of GENERATE) out[key] = checked(generate, key);
    return out;
  }

  function showError(message) {
    error.textContent = message || "";
    error.classList.toggle("hidden", !message);
  }

  function setBusy(busy) {
    for (const control of form.querySelectorAll("input, select, button")) control.disabled = busy;
  }

  function keep() {
    if (data.panel) vscode.setState({ ...data.panel, form: read() });
  }
  keep();

  form.addEventListener("click", (event) => {
    const button = event.target.closest("[data-action]");
    if (!button) return;
    switch (button.dataset.action) {
      case "add-field":
        addField().querySelector("input").focus();
        break;
      case "add-relation":
        addRelation().querySelector("select").focus();
        break;
      case "add-index":
        addIndex().querySelector("input").focus();
        break;
      case "remove":
        button.closest(".row").remove();
        break;
      case "cancel":
        vscode.postMessage({ type: "cancel" });
        return;
    }
    keep();
  });
  form.addEventListener("input", () => {
    showError("");
    keep();
  });

  // Enter in a text field is not "submit": Ctrl+Enter is.
  form.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || !event.target.matches("input[type=text]")) return;
    event.preventDefault();
    if (event.ctrlKey || event.metaKey) form.requestSubmit();
  });

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const filled = read();
    setBusy(true);
    vscode.postMessage({ type: "create", form: filled });
  });

  window.addEventListener("message", (event) => {
    if (event.data.type === "error") {
      setBusy(false);
      showError(event.data.message);
    }
  });

  name.focus();
})();
