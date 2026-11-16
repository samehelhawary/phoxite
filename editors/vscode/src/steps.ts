// A question asked in several steps, with a way back: plain logic, so it runs in the
// unit tests. `laravel/input.ts` has the inputs that show a step.

/** What the user did at a step: answered it, went back to the one before, or gave up. */
export type Outcome = "next" | "back" | "cancel";

/** Where a step is among the steps: `2` of `5`. */
export interface Position {
  step: number;
  total: number;
}

export type Step = (at: Position) => Promise<Outcome>;

/**
 * Asks `steps` in order. Going back at a step asks the one before again (a step keeps
 * what it was answered, so nothing is typed twice). Whether every step was answered.
 */
export async function runSteps(steps: readonly Step[]): Promise<boolean> {
  let index = 0;
  while (index < steps.length) {
    const outcome = await steps[index]!({ step: index + 1, total: steps.length });
    if (outcome === "cancel") return false;
    index = outcome === "back" ? Math.max(0, index - 1) : index + 1;
  }
  return true;
}
