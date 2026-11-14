import assert from "node:assert/strict";
import { test } from "node:test";
import { openable, parseRoutes, routeLabel, routePicks, type Route } from "../src/routes.ts";

const route = (over: Partial<Route> = {}): Route => ({
  name: "posts.show",
  methods: ["GET", "HEAD"],
  uri: "posts/{post}",
  action: "App\\Http\\Controllers\\PostController@show",
  middleware: ["web"],
  path: "/app/routes/web.php",
  range: { start: { line: 3, col: 0 }, end: { line: 3, col: 10 } },
  url: "http://localhost/posts/{post}",
  request: "### posts.show\nGET http://localhost/posts/{{post}}\n",
  ...over,
});

test("reads the routes of a command argument", () => {
  assert.equal(parseRoutes(JSON.stringify([route()]))[0]?.name, "posts.show");
  assert.deepEqual(parseRoutes("not json"), []);
  assert.deepEqual(parseRoutes(JSON.stringify({ uri: "/" })), []);
  assert.deepEqual(parseRoutes(42), []);
});

test("offers four actions per route", () => {
  assert.equal(routeLabel(route()), "GET|HEAD /posts/{post}");
  const one = routePicks([route()]);
  assert.deepEqual(
    one.map((p) => p.label),
    ["Go to Declaration", "Copy URL", "Open in Browser", "Copy HTTP Request"],
  );
  assert.equal(one[0]?.description, "GET|HEAD /posts/{post} · posts.show");
  const two = routePicks([route(), route({ name: null, uri: "/about", methods: ["GET"] })]);
  assert.equal(two.length, 8);
  assert.equal(two[4]?.label, "GET /about: Go to Declaration");
});

test("opens only URLs without placeholders", () => {
  assert.equal(openable("http://localhost/about"), true);
  assert.equal(openable("http://localhost/posts/{post}"), false);
  assert.equal(openable("/about"), false);
});
