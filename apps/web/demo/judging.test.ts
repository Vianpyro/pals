import assert from "node:assert/strict";
import { test } from "node:test";
import { caseVerdict, judgeCases, shown, type Case, type Outcome, type Report } from "./judging.ts";

const exited = (stdout: string, success = true): Outcome => ({ kind: "exited", success, stdout });
const visible = (input: string, expected: string): Case => ({ input, expected, visible: true });
const hidden = (input: string, expected: string): Case => ({ input, expected, visible: false });
const cases = [visible("1", "one\n"), visible("2", "two\n")];

// Answers each input with its outcome, and keeps what was run and what was reported
function runner(outcomes: Record<string, Outcome>) {
    const ran: string[] = [];
    const reported: Report[] = [];
    return {
        ran,
        reported,
        run: async (input: string) => {
            ran.push(input);
            return outcomes[input];
        },
        report: (report: Report) => void reported.push(report),
    };
}

const judge = (cases: readonly Case[], { run, report }: ReturnType<typeof runner>) => judgeCases(cases, run, report);

test("passes when every case prints what is expected", async () => {
    assert.equal(await judge(cases, runner({ 1: exited("one\n"), 2: exited("two\n") })), true);
});

test("ignores trailing whitespace, as the judge does", async () => {
    assert.equal(await judge(cases, runner({ 1: exited("one  \r\n\n\n"), 2: exited("two") })), true);
});

test("keeps leading whitespace and blank lines between lines, as the judge does", () => {
    assert.equal(caseVerdict(exited(" a\nb\n"), "a\nb\n"), "wrong-answer");
    assert.equal(caseVerdict(exited("a\n\nb\n"), "a\nb\n"), "wrong-answer");
});

test("each outcome gets the judge's verdict", () => {
    assert.equal(caseVerdict(exited("one\n"), "one\n"), "passed");
    assert.equal(caseVerdict(exited("wrong\n"), "one\n"), "wrong-answer");
    // A program that crashed fails even with the right output
    assert.equal(caseVerdict(exited("one\n", false), "one\n"), "crashed");
    assert.equal(caseVerdict({ kind: "timed-out" }, "one\n"), "timed-out");
});

test("reports each visible case as it ends, even after one fails", async () => {
    const program = runner({ 1: exited("wrong\n"), 2: exited("two\n") });
    assert.equal(await judge(cases, program), false);
    assert.deepEqual(program.reported, [
        { position: 0, verdict: "wrong-answer", actual: "wrong\n", truncated: false },
        { position: 1, verdict: "passed", actual: "two\n", truncated: false },
    ]);
});

test("never reports a hidden case", async () => {
    const program = runner({ 1: exited("one\n"), 3: exited("wrong\n") });
    assert.equal(await judge([visible("1", "one\n"), hidden("3", "three\n")], program), false);
    assert.deepEqual(
        program.reported.map(({ position }) => position),
        [0],
    );
});

test("past a failure, the hidden cases do not run: nobody sees them, the verdict is known", async () => {
    const program = runner({ 1: exited("wrong\n"), 3: exited("three\n") });
    assert.equal(await judge([visible("1", "one\n"), hidden("3", "three\n"), hidden("4", "four\n")], program), false);
    assert.deepEqual(program.ran, ["1"]);
});

test("a program that timed out shows nothing", () => {
    assert.deepEqual(shown({ kind: "timed-out" }), { actual: "", truncated: false });
});

test("an output past 64 KiB is cut and says so, without splitting a character", () => {
    const limit = 64 * 1024;
    assert.deepEqual(shown(exited("a".repeat(limit))), { actual: "a".repeat(limit), truncated: false });
    assert.deepEqual(shown(exited("a".repeat(limit + 1))), { actual: "a".repeat(limit), truncated: true });
    // "é" is two bytes in UTF-8: the one across the limit is left out
    assert.deepEqual(shown(exited("a" + "é".repeat(limit / 2))), { actual: "a" + "é".repeat(limit / 2 - 1), truncated: true });
});

test("a NUL byte is replaced, as the judge does for the database", () => {
    assert.deepEqual(shown(exited("a\0b")), { actual: "a�b", truncated: false });
});
