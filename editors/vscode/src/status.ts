// What the status item says about the server: plain logic, so it runs in the unit tests.

/** What the extension knows about the server. */
export interface ServerStatus {
  /** `failed`: it didn't start; `stopped`: it ran and no longer does. */
  server: "starting" | "running" | "stopped" | "failed";
  /** The server is reading the project. */
  indexing: boolean;
  /** The workspace is a Laravel project, as far as the server said. */
  laravel: boolean;
  /** The frameworks of the open projects, as the server names them (`WordPress`). */
  frameworks?: readonly string[];
  /** The server's version, once it said it. */
  version?: string;
  /** What the last indexing found: `1204 files, 9310 declarations`. */
  indexed?: string;
  /** Why it didn't start. */
  problem?: string;
}

export interface StatusView {
  text: string;
  detail: string;
  busy: boolean;
  severity: "information" | "error";
}

/** `Phoxite: Laravel`, `Phoxite: Symfony, WordPress`, or `Phoxite` in a project of no framework. */
function frameworksText(status: ServerStatus): string {
  // A server that doesn't name the frameworks still says whether the project is a Laravel one.
  const names = status.frameworks?.length ? status.frameworks : status.laravel ? ["Laravel"] : [];
  return names.length ? `Phoxite: ${names.join(", ")}` : "Phoxite";
}

export function statusView(status: ServerStatus): StatusView {
  const version = status.version ? ` ${status.version}` : "";
  switch (status.server) {
    case "starting":
      return { text: "Phoxite: starting", detail: "The language server is starting", busy: true, severity: "information" };
    case "failed":
      return {
        text: "Phoxite: not running",
        detail: status.problem ?? "The language server didn't start",
        busy: false,
        severity: "error",
      };
    case "stopped":
      return { text: "Phoxite: stopped", detail: "The language server stopped", busy: false, severity: "error" };
    case "running": {
      if (status.indexing) {
        return { text: "Phoxite: indexing", detail: "Reading the project's PHP files", busy: true, severity: "information" };
      }
      const parts = [`phoxite-lsp${version}`];
      if (status.indexed) parts.push(status.indexed);
      return {
        text: frameworksText(status),
        detail: parts.join(" · "),
        busy: false,
        severity: "information",
      };
    }
  }
}
