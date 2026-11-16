// What a failed request tells the user, in one place.

import * as vscode from "vscode";
import type { LanguageClient } from "vscode-languageclient/node";

/** What went wrong, as the server or the editor put it. */
export const message = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** The client when its server runs; otherwise an error whose message is for the user. */
export function running(client: LanguageClient | undefined): LanguageClient {
  if (!client?.isRunning()) {
    throw new Error("The Phoxite language server isn't running. Run \"Phoxite: Restart the Language Server\".");
  }
  return client;
}

/** The answer of a request that failed: not the same as "nothing was found". */
export const FAILED = Symbol("failed");

/**
 * Awaits `request`. A failure is shown to the user and answered with [`FAILED`], so the
 * caller doesn't mistake a server that is down for a project without what it asked for.
 */
export async function shown<T>(request: Promise<T>): Promise<T | typeof FAILED> {
  try {
    return await request;
  } catch (e) {
    void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
    return FAILED;
  }
}

/** Runs a command's body; what it throws is shown instead of VS Code's generic error. */
export function guarded<A extends unknown[]>(run: (...args: A) => unknown): (...args: A) => Promise<void> {
  return async (...args: A) => {
    try {
      await run(...args);
    } catch (e) {
      void vscode.window.showErrorMessage(`Phoxite: ${message(e)}`);
    }
  };
}
