// The Change Signature form: the name, visibility and return type, and a row per
// parameter (reordered, added and removed here). Refactor posts the signature to
// the extension, which checks it and has the server compute the edit.
(function () {
  const vscode = acquireVsCodeApi();
  const data = JSON.parse(document.getElementById("data").textContent);
  const was = data.signature;
  // What was typed before the tab was hidden, or the window reloaded.
  const saved = vscode.getState();
  const start = (saved && saved.edited) || was;
  const form = document.getElementById("signature");
  const head = document.getElementById("head");
  const rows = document.getElementById("parameters");
  const none = document.getElementById("none");
  const preview = document.getElementById("preview");
  const error = document.getElementById("error");

  function element(tag, props, children) {
    const node = Object.assign(document.createElement(tag), props || {});
    for (const child of children || []) node.append(child);
    return node;
  }

  function input(name, placeholder, value, className) {
    const node = element("input", { type: "text", name, placeholder, value: value || "", className: className || "" });
    node.setAttribute("aria-label", placeholder);
    node.spellcheck = false;
    return node;
  }

  function check(name, label, checked) {
    return element("label", { className: "check" }, [element("input", { type: "checkbox", name, checked: !!checked }), label]);
  }

  function button(action, text, title) {
    const node = element("button", { type: "button", className: "remove", textContent: text, title });
    node.dataset.action = action;
    node.setAttribute("aria-label", title);
    return node;
  }

  // The name, and for a method its visibility; a constructor has no return type.
  const name = input("functionName", "Name", start.functionName);
  name.disabled = was.constructor;
  head.append(name);
  let visibility;
  if (was.visibility) {
    visibility = element(
      "select",
      { name: "visibility" },
      data.visibilities.map((v) => element("option", { value: v, textContent: v })),
    );
    visibility.value = start.visibility || was.visibility;
    visibility.setAttribute("aria-label", "Visibility");
    head.prepend(visibility);
  }
  let returnType;
  if (!was.constructor) {
    returnType = input("returnType", "Return type", start.returnType, "narrow");
    head.append(returnType);
  }

  function addRow(p) {
    const row = element("div", { className: "row" }, [
      input("type", "Type", p.type, "narrow"),
      input("name", "$name", p.name ? "$" + p.name : ""),
      input("defaultValue", "Default value", p.defaultValue, "narrow"),
    ]);
    row.dataset.oldIndex = p.oldIndex === null || p.oldIndex === undefined ? "" : String(p.oldIndex);
    row.dataset.promoted = p.promoted ? "1" : "";
    if (row.dataset.oldIndex === "") {
      // A new parameter: what the existing calls pass for it.
      row.append(input("callValue", "Value in existing calls", p.callValue, "narrow"));
    }
    row.append(
      check("byRef", "By reference", p.byRef),
      check("variadic", "Variadic", p.variadic),
      button("up", "↑", "Move the parameter up"),
      button("down", "↓", "Move the parameter down"),
    );
    if (p.promoted) {
      // A promoted property keeps its name and its place in the class.
      row.querySelector("[name=name]").disabled = true;
      row.append(element("span", { className: "hint", textContent: "promoted" }));
    } else {
      row.append(button("remove", "✕", "Remove the parameter"));
    }
    rows.append(row);
    return row;
  }

  function signature() {
    return {
      name: was.name,
      functionName: name.value,
      visibility: visibility ? visibility.value : "",
      returnType: returnType ? returnType.value : "",
      constructor: was.constructor,
      parameters: [...rows.children].map((row) => {
        const field = (n) => row.querySelector("[name=" + n + "]");
        return {
          oldIndex: row.dataset.oldIndex === "" ? null : Number(row.dataset.oldIndex),
          name: field("name").value.trim().replace(/^\$/, ""),
          type: field("type").value,
          defaultValue: field("defaultValue").value,
          callValue: field("callValue") ? field("callValue").value : "",
          byRef: field("byRef").checked,
          variadic: field("variadic").checked,
          promoted: row.dataset.promoted === "1",
        };
      }),
    };
  }

  function showError(message) {
    error.textContent = message || "";
    error.classList.toggle("hidden", !message);
  }

  function refresh() {
    const s = signature();
    const parameters = s.parameters.map((p) => {
      const variable = (p.byRef ? "&" : "") + (p.variadic ? "..." : "") + "$" + p.name;
      return (p.type.trim() ? p.type.trim() + " " : "") + variable + (p.defaultValue.trim() ? " = " + p.defaultValue.trim() : "");
    });
    let text = (s.visibility ? s.visibility + " " : "") + "function " + s.functionName.trim() + "(" + parameters.join(", ") + ")";
    if (s.returnType.trim()) text += ": " + s.returnType.trim();
    preview.textContent = text;
    none.classList.toggle("hidden", rows.children.length > 0);
    if (data.panel) vscode.setState({ ...data.panel, edited: s });
    const all = [...rows.children];
    all.forEach((row, i) => {
      row.querySelector("[data-action=up]").disabled = i === 0;
      row.querySelector("[data-action=down]").disabled = i === all.length - 1;
    });
  }

  function setBusy(busy) {
    for (const control of form.querySelectorAll("button")) control.disabled = busy;
    if (!busy) refresh();
  }

  for (const p of start.parameters) addRow(p);
  refresh();
  (was.constructor ? rows.querySelector("input") || form.querySelector("button") : name).focus();

  form.addEventListener("input", () => {
    showError("");
    refresh();
  });

  // Enter in a text field is not "submit": Ctrl+Enter is.
  form.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" || !event.target.matches("input[type=text]")) return;
    event.preventDefault();
    if (event.ctrlKey || event.metaKey) form.requestSubmit();
  });

  form.addEventListener("click", (event) => {
    const control = event.target.closest("[data-action]");
    if (!control) return;
    const row = control.closest(".row");
    switch (control.dataset.action) {
      case "add":
        addRow({ oldIndex: null, name: "", type: "", defaultValue: "", callValue: "" })
          .querySelector("[name=type]")
          .focus();
        break;
      case "remove":
        row.remove();
        break;
      case "up":
        if (row.previousElementSibling) rows.insertBefore(row, row.previousElementSibling);
        control.focus();
        break;
      case "down":
        if (row.nextElementSibling) rows.insertBefore(row.nextElementSibling, row);
        control.focus();
        break;
      case "cancel":
        vscode.postMessage({ type: "cancel" });
        return;
    }
    showError("");
    refresh();
  });

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    setBusy(true);
    vscode.postMessage({ type: "apply", signature: signature() });
  });

  window.addEventListener("message", (event) => {
    if (event.data.type === "error") {
      setBusy(false);
      showError(event.data.message);
    }
  });
})();
