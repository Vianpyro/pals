// The visible cases of each run, as the event stream reports them (ADR-0030).
import type { CaseResult } from "@log791/contracts/CaseResult";
import type { Run } from "@log791/contracts/Run";

/** By run id, in order of position */
export type Cases = Record<string, readonly CaseResult[]>;

/** One result per position: every new connection replays the cases reported so far. */
export function withCase(cases: Cases, result: CaseResult): Cases {
    const others = (cases[result.run] ?? []).filter(({ position }) => position !== result.position);
    return { ...cases, [result.run]: [...others, result].sort((a, b) => a.position - b.position) };
}

/** A run back to waiting was reclaimed after a judge failure: the judge reports its cases again. */
export function withRun(cases: Cases, run: Run): Cases {
    if (run.state !== "waiting" || !(run.id in cases)) return cases;
    const { [run.id]: _, ...rest } = cases;
    return rest;
}
