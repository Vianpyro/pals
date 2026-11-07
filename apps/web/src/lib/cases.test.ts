import assert from "node:assert/strict";
import { test } from "node:test";
import type { CaseResult } from "@log791/contracts/CaseResult";
import type { Run } from "@log791/contracts/Run";
import { withCase, withRun } from "./cases.ts";

const result = (position: number, verdict: CaseResult["verdict"] = "passed", run = "a"): CaseResult => ({
    run,
    position,
    verdict,
    actual: "",
    truncated: false,
});
const run = (state: "waiting" | "running"): Run => ({ id: "a", kind: "check", state, enqueued_at: "2026-01-01T09:00:00Z" });

test("keeps each run's cases in order, whatever order they arrive in", () => {
    const cases = withCase(withCase(withCase({}, result(1)), result(0)), result(0, "passed", "b"));
    assert.deepEqual(cases, { a: [result(0), result(1)], b: [result(0, "passed", "b")] });
});

test("a case received again, as on a reconnection, replaces the one it repeats", () => {
    const cases = withCase(withCase({}, result(0, "wrong-answer")), result(0, "passed"));
    assert.deepEqual(cases, { a: [result(0, "passed")] });
});

test("a run back to waiting loses its cases: the judge reports them again", () => {
    const cases = withCase({}, result(0));
    assert.deepEqual(withRun(cases, run("waiting")), {});
    assert.equal(withRun(cases, run("running")), cases);
});
