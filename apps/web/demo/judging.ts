// A program's verdict on an exercise's cases, by the rules of the judge (apps/judge/src/stdio.rs).

export type Case = { input: string; expected: string };

/** What a program did on one input: as a process would have exited, or still running at the time limit. */
export type Outcome = { kind: "exited"; success: boolean; stdout: string } | { kind: "timed-out" };

// The whitespace of Rust's trim_ascii_end, which the judge compares with
const trimEnd = (text: string) => text.replace(/[ \t\n\f\r]+$/, "");
const lines = (output: string) => trimEnd(output).split("\n").map(trimEnd);

function sameOutput(expected: string, actual: string): boolean {
    const wanted = lines(expected);
    const got = lines(actual);
    return wanted.length === got.length && wanted.every((line, index) => line === got[index]);
}

const passes = (outcome: Outcome, expected: string) =>
    outcome.kind === "exited" && outcome.success && sameOutput(expected, outcome.stdout);

/** Whether every case passes. Stops at the first that fails: the page only shows whether all pass. */
export async function judgeCases(cases: readonly Case[], run: (input: string) => Promise<Outcome>): Promise<boolean> {
    for (const { input, expected } of cases) {
        if (!passes(await run(input), expected)) return false;
    }
    return true;
}
