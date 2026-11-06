import assert from "node:assert/strict";
import { test } from "node:test";
import { judgeCases, type Outcome } from "./judging.ts";

const exited = (stdout: string, success = true): Outcome => ({ kind: "exited", success, stdout });
const cases = [
    { input: "1", expected: "one\n" },
    { input: "2", expected: "two\n" },
];

// Answers each input with its outcome, and keeps what was run
function runner(outcomes: Record<string, Outcome>) {
    const ran: string[] = [];
    return {
        ran,
        run: async (input: string) => {
            ran.push(input);
            return outcomes[input];
        },
    };
}

test("passes when every case prints what is expected", async () => {
    const { run } = runner({ 1: exited("one\n"), 2: exited("two\n") });
    assert.equal(await judgeCases(cases, run), true);
});

test("ignores trailing whitespace, as the judge does", async () => {
    const { run } = runner({ 1: exited("one  \r\n\n\n"), 2: exited("two") });
    assert.equal(await judgeCases(cases, run), true);
});

test("keeps leading whitespace and blank lines between lines, as the judge does", async () => {
    const expected = [{ input: "1", expected: "a\nb\n" }];
    assert.equal(await judgeCases(expected, runner({ 1: exited(" a\nb\n") }).run), false);
    assert.equal(await judgeCases(expected, runner({ 1: exited("a\n\nb\n") }).run), false);
});

test("a wrong output fails, and the next cases do not run", async () => {
    const { ran, run } = runner({ 1: exited("wrong\n"), 2: exited("two\n") });
    assert.equal(await judgeCases(cases, run), false);
    assert.deepEqual(ran, ["1"]);
});

test("a program that crashed fails even with the right output", async () => {
    const { run } = runner({ 1: exited("one\n", false), 2: exited("two\n") });
    assert.equal(await judgeCases(cases, run), false);
});

test("a program that timed out fails", async () => {
    const { run } = runner({ 1: { kind: "timed-out" }, 2: exited("two\n") });
    assert.equal(await judgeCases(cases, run), false);
});
