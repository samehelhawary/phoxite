// Other extensions that answer what Phoxite answers: plain logic, so it runs in the
// unit tests. The ids are the ones the Marketplace lists.

/** What an extension doubles. */
export type Overlap = "server" | "laravel" | "blade";

export interface Other {
  /** The Marketplace id; VS Code compares ids without regard to case. */
  id: string;
  name: string;
  overlap: Overlap;
}

export const OTHERS: readonly Other[] = [
  { id: "bmewburn.vscode-intelephense-client", name: "PHP Intelephense", overlap: "server" },
  { id: "DEVSENSE.phptools-vscode", name: "PHP (DEVSENSE)", overlap: "server" },
  { id: "zobo.php-intellisense", name: "PHP IntelliSense (zobo)", overlap: "server" },
  { id: "felixfbecker.php-intellisense", name: "PHP IntelliSense (felixfbecker)", overlap: "server" },
  { id: "phpactor.vscode-phpactor", name: "Phpactor", overlap: "server" },
  { id: "laravel.vscode-laravel", name: "Laravel", overlap: "laravel" },
  { id: "amiralizadeh9480.laravel-extra-intellisense", name: "Laravel Extra Intellisense", overlap: "laravel" },
  { id: "onecentlin.laravel-blade", name: "Laravel Blade Snippets", overlap: "blade" },
  { id: "cjhowe7.laravel-blade", name: "laravel-blade", overlap: "blade" },
];

/** `A`, `A and B`, `A, B and C`. */
export function listed(names: readonly string[]): string {
  if (names.length <= 1) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names[names.length - 1]}`;
}

/**
 * What to tell the user about the other extensions that are enabled, one sentence per
 * kind of overlap; `undefined` when there is nothing to say.
 */
export function conflictMessage(found: readonly Other[]): string | undefined {
  const of = (overlap: Overlap) => found.filter((o) => o.overlap === overlap).map((o) => o.name);
  const verb = (names: string[], one: string, several: string) => (names.length === 1 ? one : several);
  const sentences: string[] = [];
  const servers = of("server");
  if (servers.length) {
    sentences.push(
      `${listed(servers)} ${verb(servers, "is", "are")} enabled too, so completions and problems show twice.`,
    );
  }
  const laravel = of("laravel");
  if (laravel.length) {
    sentences.push(`${listed(laravel)} also ${verb(laravel, "completes", "complete")} Laravel names, so those show twice.`);
  }
  const blade = of("blade");
  if (blade.length) {
    sentences.push(
      `${listed(blade)} also ${verb(blade, "defines", "define")} the Blade language, so its highlighting may be the one you see.`,
    );
  }
  if (!sentences.length) return undefined;
  return `Phoxite: ${sentences.join(" ")} Disable what you don't need for this workspace.`;
}

/** What is remembered once the user saw the warning: it shows again when the set changes. */
export const seenKey = (found: readonly Other[]) =>
  found
    .map((o) => o.id.toLowerCase())
    .sort()
    .join(",");
