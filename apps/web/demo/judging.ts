// A program's verdict on an exercise's cases, by the rules of the judge (apps/judge/src/stdio.rs),
// and what the student is told of each visible case as it ends (apps/judge/src/report.rs).
import type { CaseVerdict } from "@log791/contracts/CaseVerdict";

export type Case = { input: string; expected: string; visible: boolean };

/** What a program did on one input: as a process would have exited, or still running at the time limit. */
export type Outcome = { kind: "exited"; success: boolean; stdout: string } | { kind: "timed-out" };

/** A visible case as the judge reports it, without its run */
export type Report = { position: number; verdict: CaseVerdict; actual: string; truncated: boolean };

const MAX_SHOWN = 64 * 1024;

// The whitespace of Rust's trim_ascii_end, which the judge compares with
const trimEnd = (text: string) => text.replace(/[ \t\n\f\r]+$/, "");
const lines = (output: string) => trimEnd(output).split("\n").map(trimEnd);

function sameOutput(expected: string, actual: string): boolean {
    const wanted = lines(expected);
    const got = lines(actual);
    return wanted.length === got.length && wanted.every((line, index) => line === got[index]);
}

export function caseVerdict(outcome: Outcome, expected: string): CaseVerdict {
    if (outcome.kind === "timed-out") return "timed-out";
    if (!outcome.success) return "crashed";
    return sameOutput(expected, outcome.stdout) ? "passed" : "wrong-answer";
}

/** The output as the judge keeps it: NUL replaced, at most 64 KiB of UTF-8, never half a character. */
export function shown(outcome: Outcome): { actual: string; truncated: boolean } {
    if (outcome.kind === "timed-out") return { actual: "", truncated: false };
    const bytes = new TextEncoder().encode(outcome.stdout.replaceAll("\0", "�"));
    if (bytes.length <= MAX_SHOWN) return { actual: new TextDecoder().decode(bytes), truncated: false };
    let end = MAX_SHOWN;
    // Back to the first byte of the character the limit falls in: UTF-8 continuation bytes are 10xxxxxx
    while ((bytes[end] & 0xc0) === 0x80) end--;
    return { actual: new TextDecoder().decode(bytes.subarray(0, end)), truncated: true };
}

/**
 * Whether every case passes. Each visible case runs and is reported as it ends, as on the judge;
 * past a failure the hidden cases do not run, since nobody sees them and the verdict is known.
 */
export async function judgeCases(
    cases: readonly Case[],
    run: (input: string) => Promise<Outcome>,
    report: (report: Report) => void,
): Promise<boolean> {
    let passed = true;
    for (const [position, { input, expected, visible }] of cases.entries()) {
        if (!passed && !visible) continue;
        const outcome = await run(input);
        const verdict = caseVerdict(outcome, expected);
        if (visible) report({ position, verdict, ...shown(outcome) });
        passed &&= verdict === "passed";
    }
    return passed;
}
