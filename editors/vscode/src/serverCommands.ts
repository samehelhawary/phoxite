// The commands the server's lenses, hints and actions carry and leave to the client:
// plain logic, so it runs in the unit tests.
//
// The server names them without a prefix (`laravel.modelView`), as other editors read
// them from `docs/editors.md`. VS Code has one namespace for the commands of every
// extension, so the extension registers them under names of its own and rewrites what
// the server sends: another extension registering `laravel.modelView` can then neither
// break this one's activation nor take its lenses.

/** The server's commands that the client runs. */
export const SERVER_COMMANDS = [
  "laravel.routeActions",
  "laravel.modelView",
  "laravel.runArtisan",
  "laravel.createTranslation",
] as const;

export type ServerCommand = (typeof SERVER_COMMANDS)[number];

/** The name the extension registers `command` under. */
export const ownName = (command: ServerCommand) => `phoxite.server.${command}`;

/** `command` as VS Code should run it: the extension's own name for one of the server's. */
export function rewritten(command: string): string {
  return (SERVER_COMMANDS as readonly string[]).includes(command) ? ownName(command as ServerCommand) : command;
}

/** Rewrites the command a lens, hint part or action carries, in place; gives it back. */
export function withOwnCommand<T extends { command?: { command: string } | string | null }>(item: T): T {
  const carried = item.command;
  if (carried && typeof carried === "object" && typeof carried.command === "string") {
    carried.command = rewritten(carried.command);
  }
  return item;
}
