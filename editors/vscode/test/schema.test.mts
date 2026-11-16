import assert from "node:assert/strict";
import { test } from "node:test";
import { newModelPage } from "../src/laravel/html.ts";
import {
  COLUMN_TYPES,
  MODEL_COLUMN_TYPES,
  type NewModelForm,
  RELATION_KINDS,
  emptyModelForm,
  newModelError,
  newModelSpec,
} from "../src/laravel/model.ts";

const form = (over: Partial<NewModelForm>): NewModelForm => ({ ...emptyModelForm(), name: "Ticket", ...over });
const field = (name: string, ty: string, over: object = {}) => ({
  name,
  ty,
  nullable: false,
  unique: false,
  index: false,
  default: "",
  ...over,
});

test("an enum sends its values, a foreign key what deleting does; other types send neither", () => {
  const spec = newModelSpec(
    form({
      fields: [
        field("status", "enum", { values: " draft, published ,, archived" }),
        field("team_id", "foreignUuid", { onDelete: "cascade" }),
        // What a hidden input still holds is not sent.
        field("title", "string", { values: "a, b", onDelete: "cascade" }),
        field("subject", "morphs"),
      ],
    }),
  );
  assert.deepEqual(
    spec.fields.map((f) => [f.name, f.ty, f.values, f.onDelete]),
    [
      ["status", "enum", ["draft", "published", "archived"], ""],
      ["team_id", "foreignUuid", [], "cascade"],
      ["title", "string", [], ""],
      ["subject", "morphs", [], ""],
    ],
  );
});

test("the form says what an enum, a foreign key and a relation through a model still need", () => {
  assert.equal(
    newModelError(form({ fields: [field("status", "enum")] })),
    "List the values of the enum column 'status' (draft, published)",
  );
  assert.equal(
    newModelError(form({ fields: [field("user_id", "foreignId", { onDelete: "setNull" })] })),
    "Make 'user_id' nullable: it is set to null on delete",
  );
  assert.equal(newModelError(form({ fields: [field("user_id", "foreignId", { onDelete: "setNull", nullable: true })] })), null);
  assert.equal(
    newModelError(form({ fields: [field("user_id", "foreignId", { onDelete: "explode" })] })),
    "'explode' is not something a foreign key does on delete",
  );
  const through = { kind: "hasManyThrough", model: "App\\Models\\Post", name: "" };
  assert.equal(newModelError(form({ relations: [through] })), "Choose the model the relation to Post goes through");
  assert.equal(newModelError(form({ relations: [{ ...through, through: "App\\Models\\User" }] })), null);
  // Only a hasManyThrough goes through a model.
  const spec = newModelSpec(form({ relations: [{ kind: "morphOne", model: "Image", name: "", through: "User" }] }));
  assert.deepEqual(spec.relations, [{ kind: "morphOne", model: "Image", name: "", through: "" }]);
});

test("the New Model form offers the types Add Columns has, and those that need more than a name", () => {
  assert.deepEqual(
    MODEL_COLUMN_TYPES.filter((ty) => !(COLUMN_TYPES as readonly string[]).includes(ty)),
    ["enum", "morphs"],
  );
  for (const ty of ["char", "longText", "tinyInteger", "unsignedInteger", "time", "ulid", "foreignUuid", "ipAddress"]) {
    assert.ok((COLUMN_TYPES as readonly string[]).includes(ty), ty);
  }
  for (const kind of ["hasManyThrough", "morphOne", "morphToMany"]) {
    assert.ok((RELATION_KINDS as readonly string[]).includes(kind), kind);
  }
  const ctx = { namespace: "App", modules: [], models: [], defaultModule: null, generators: [] };
  const page = newModelPage(ctx as never, emptyModelForm(), { cspSource: "x:", style: "a.css", script: "a.js" });
  assert.match(page, /"enum","morphs"\]/);
  assert.match(page, /"onDelete":\[\{"value":"","label":"On delete: database default"\}/);
  assert.match(page, /"foreignTypes":\["foreignId","foreignUuid"\]/);
});
