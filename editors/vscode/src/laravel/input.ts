// The inputs of a question asked in several steps (`steps.ts`): each shows which step
// it is and, after the first, a Back button.

import * as vscode from "vscode";
import type { Outcome, Position } from "../steps";

/** A step's place and the title all steps share. */
export interface At extends Position {
  title: string;
}

export type Choice<T> = vscode.QuickPickItem & { value: T };

/** What a step answers: the value, or how the user left it. */
export type Answer<T> = { value: T } | Exclude<Outcome, "next">;

function place(input: vscode.QuickInput, at: At): void {
  input.title = at.title;
  // One step is not "step 1 of 1".
  if (at.total > 1) {
    input.step = at.step;
    input.totalSteps = at.total;
  }
  input.ignoreFocusOut = true;
}

const back = (at: At) => (at.step > 1 ? [vscode.QuickInputButtons.Back] : []);

/** Shows `input` until `settle` is called; hiding it is giving up. */
function shown<T>(input: vscode.QuickInput, wire: (settle: (answer: Answer<T>) => void) => void): Promise<Answer<T>> {
  return new Promise((resolve) => {
    let settled = false;
    const settle = (answer: Answer<T>) => {
      if (settled) return;
      settled = true;
      resolve(answer);
      input.dispose();
    };
    wire(settle);
    input.onDidHide(() => settle("cancel"));
    input.show();
  });
}

/** Asks for a text; `validate` answers what is wrong with it. */
export function askText(
  at: At,
  options: { prompt: string; value?: string; validate?: (value: string) => string | null | undefined },
): Promise<Answer<string>> {
  const box = vscode.window.createInputBox();
  place(box, at);
  box.prompt = options.prompt;
  box.value = options.value ?? "";
  box.buttons = back(at);
  return shown(box, (settle) => {
    box.onDidChangeValue(() => (box.validationMessage = undefined));
    box.onDidAccept(() => {
      const problem = options.validate?.(box.value);
      if (problem) box.validationMessage = problem;
      else settle({ value: box.value });
    });
    box.onDidTriggerButton(() => settle("back"));
  });
}

/** Asks for one of `items`; `current` (what the step was answered before) is the active one. */
export function askChoice<T>(at: At, options: { placeHolder: string; items: Array<Choice<T>>; current?: T }): Promise<Answer<T>> {
  const pick = vscode.window.createQuickPick<Choice<T>>();
  place(pick, at);
  pick.placeholder = options.placeHolder;
  pick.matchOnDescription = true;
  pick.items = options.items;
  const active = options.items.find((item) => item.value === options.current);
  if (active) pick.activeItems = [active];
  pick.buttons = back(at);
  return shown(pick, (settle) => {
    pick.onDidAccept(() => {
      const chosen = pick.selectedItems[0] ?? pick.activeItems[0];
      if (chosen) settle({ value: chosen.value });
    });
    pick.onDidTriggerButton(() => settle("back"));
  });
}

/** Asks which of `items` are ticked; those with `picked` are ticked to begin with. */
export function askTicks<T>(at: At, options: { placeHolder: string; items: Array<Choice<T>> }): Promise<Answer<T[]>> {
  const pick = vscode.window.createQuickPick<Choice<T>>();
  place(pick, at);
  pick.placeholder = options.placeHolder;
  pick.canSelectMany = true;
  pick.items = options.items;
  pick.selectedItems = options.items.filter((item) => item.picked);
  pick.buttons = back(at);
  return shown(pick, (settle) => {
    pick.onDidAccept(() => settle({ value: pick.selectedItems.map((item) => item.value) }));
    pick.onDidTriggerButton(() => settle("back"));
  });
}
