// The Eloquent model view's page: its links go to their declarations, and the
// Add Columns form posts its rows to the extension, which checks and applies them.
(function () {
  const vscode = acquireVsCodeApi();
  const data = JSON.parse(document.getElementById("data").textContent);
  const details = document.getElementById("details");
  const form = document.getElementById("add-columns");
  const openForm = document.getElementById("open-form");
  const rows = document.getElementById("rows");
  const error = document.getElementById("error");

  function element(tag, props, children) {
    const node = Object.assign(document.createElement(tag), props || {});
    for (const child of children || []) node.append(child);
    return node;
  }

  function addRow() {
    const name = element("input", { type: "text", placeholder: "Column name", name: "name" });
    name.setAttribute("aria-label", "Column name");
    const type = element(
      "select",
      { name: "ty" },
      data.columnTypes.map((ty) => element("option", { value: ty, textContent: ty })),
    );
    type.setAttribute("aria-label", "Column type");
    const nullable = element("label", { className: "check" }, [
      element("input", { type: "checkbox", name: "nullable" }),
      "Nullable",
    ]);
    const remove = element("button", { type: "button", className: "remove", textContent: "✕", title: "Remove the column" });
    remove.dataset.action = "remove-row";
    rows.append(element("div", { className: "row" }, [name, type, nullable, remove]));
    return name;
  }

  function showError(message) {
    error.textContent = message || "";
    error.classList.toggle("hidden", !message);
  }

  function showForm(show) {
    if (!form) return;
    form.classList.toggle("hidden", !show);
    openForm.classList.toggle("hidden", show);
    showError("");
    if (show) {
      rows.replaceChildren();
      addRow().focus();
    }
  }

  function setBusy(busy) {
    for (const control of form.querySelectorAll("input, select, button")) control.disabled = busy;
  }

  document.addEventListener("click", (event) => {
    const link = event.target.closest("[data-place]");
    if (link) {
      event.preventDefault();
      vscode.postMessage({ type: "goto", place: JSON.parse(link.dataset.place) });
      return;
    }
    const button = event.target.closest("[data-action]");
    if (!button) return;
    switch (button.dataset.action) {
      case "show-form":
        showForm(true);
        break;
      case "hide-form":
        showForm(false);
        break;
      case "add-row":
        addRow().focus();
        break;
      case "remove-row":
        if (rows.children.length > 1) button.closest(".row").remove();
        break;
    }
  });

  if (form) {
    form.addEventListener("input", () => showError(""));
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const columns = [...rows.children].map((row) => ({
        name: row.querySelector("[name=name]").value,
        ty: row.querySelector("[name=ty]").value,
        nullable: row.querySelector("[name=nullable]").checked,
      }));
      setBusy(true);
      vscode.postMessage({ type: "addColumns", columns, fillable: document.getElementById("fillable").checked });
    });
  }

  window.addEventListener("message", (event) => {
    const message = event.data;
    switch (message.type) {
      // The project changed: what the view shows, read again.
      case "details":
        details.innerHTML = message.html;
        break;
      case "error":
        setBusy(false);
        showError(message.message);
        break;
      case "added":
        setBusy(false);
        showForm(false);
        break;
    }
  });
})();
