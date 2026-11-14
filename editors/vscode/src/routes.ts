// Laravel route actions: what the route lenses and hints of the server offer
// (`laravel.routeActions`, with the routes as JSON).

/** A route as the server describes it (`RouteInfo`). */
export interface Route {
  name: string | null;
  methods: string[];
  uri: string;
  action: string;
  middleware: string[];
  path: string;
  range: { start: { line: number; col: number }; end: { line: number; col: number } };
  /** The URL with the app's `APP_URL`. */
  url: string;
  /** An HTTP request for the route, in `.http` file syntax. */
  request: string;
  domain?: string | null;
}

export type RouteAction = "goto" | "copy" | "open" | "request";

export interface RoutePick {
  label: string;
  description?: string;
  route: Route;
  action: RouteAction;
}

/** The routes of a command argument; nothing when it isn't route JSON. */
export function parseRoutes(arg: unknown): Route[] {
  if (typeof arg !== "string") return [];
  try {
    const parsed: unknown = JSON.parse(arg);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (r): r is Route =>
        typeof r === "object" && r !== null && typeof r.uri === "string" && typeof r.path === "string",
    );
  } catch {
    return [];
  }
}

/** `GET|HEAD /posts/{post}`. */
export function routeLabel(route: Route): string {
  const uri = route.uri.startsWith("/") ? route.uri : `/${route.uri}`;
  return `${route.methods.join("|")} ${uri}`;
}

/** The quick pick entries: the four actions of each route. */
export function routePicks(routes: Route[]): RoutePick[] {
  const several = routes.length > 1;
  const picks: RoutePick[] = [];
  for (const route of routes) {
    const prefix = several ? `${routeLabel(route)}: ` : "";
    const description = several ? (route.name ?? undefined) : `${routeLabel(route)}${route.name ? ` · ${route.name}` : ""}`;
    picks.push(
      { label: `${prefix}Go to Declaration`, description, route, action: "goto" },
      { label: `${prefix}Copy URL`, description: route.url, route, action: "copy" },
      { label: `${prefix}Open in Browser`, description: route.url, route, action: "open" },
      { label: `${prefix}Copy HTTP Request`, description, route, action: "request" },
    );
  }
  return picks;
}

/** A URL can be opened when it has no placeholder left (`{post}`). */
export function openable(url: string): boolean {
  return /^https?:\/\//.test(url) && !/\{[^}]*\}/.test(url);
}
