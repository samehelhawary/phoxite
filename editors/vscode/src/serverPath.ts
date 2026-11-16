// Where the language server's executable is: plain logic, so it runs in the unit tests.

import * as path from "node:path";

/** What `phoxite.server.path` is resolved against. */
export interface PathContext {
  /** The user's home folder, for a leading `~`. */
  home: string;
  /** The first workspace folder, for a relative path; none in an empty window. */
  folder: string | undefined;
}

/**
 * The executable `phoxite.server.path` names, as an absolute path: `~` is the home
 * folder, a relative path is relative to the workspace folder. `undefined` when the
 * setting is empty (the bundled server is used).
 */
export function configuredServer(setting: string | undefined, context: PathContext): string | undefined {
  const typed = setting?.trim();
  if (!typed) return undefined;
  if (typed === "~" || typed.startsWith("~/") || typed.startsWith("~\\")) {
    return path.join(context.home, typed.slice(1));
  }
  if (path.isAbsolute(typed) || !context.folder) return typed;
  return path.resolve(context.folder, typed);
}

/** The file name of the server bundled for `platform`. */
export const bundledName = (platform: NodeJS.Platform) => (platform === "win32" ? "phoxite-lsp.exe" : "phoxite-lsp");

/** Why the server can't be started, in the words the user reads; `null` when it can. */
export function serverProblem(
  configured: string | undefined,
  bundled: string,
  exists: (file: string) => boolean,
): string | null {
  if (configured) {
    return exists(configured) ? null : `\`phoxite.server.path\` names ${configured}, which doesn't exist.`;
  }
  return exists(bundled)
    ? null
    : "this package has no language server for your platform. Set `phoxite.server.path` to a phoxite-lsp executable.";
}
