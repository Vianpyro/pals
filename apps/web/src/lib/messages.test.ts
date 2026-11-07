import assert from "node:assert/strict";
import { test } from "node:test";
import type { Translate } from "./i18n.ts";
import { availabilityText, caseText, failureText, runText, saveText } from "./messages.ts";

// Shows which key and values were asked for, so the tests check the decision, not the wording
const t: Translate = (key, values) => (values ? `${key} ${JSON.stringify(values)}` : key);

test("a known status has its own message", () => {
    assert.equal(failureText(t, { kind: "status", status: 404 }, { 404: "catalog.unknownCourse" }), "catalog.unknownCourse");
});

test("an API restarting behind nginx says so, wherever it happens", () => {
    assert.equal(failureText(t, { kind: "status", status: 502 }), "server.starting");
});

test("another status is shown by its number", () => {
    assert.equal(failureText(t, { kind: "status", status: 500 }), 'server.error {"status":500}');
});

test("no answer at all is the server out of reach", () => {
    assert.equal(failureText(t, { kind: "unreachable" }), "server.unreachable");
});

test("a judged run says whether it passed", () => {
    assert.equal(runText(t, { state: "done", passed: true }), "run.passed");
    assert.equal(runText(t, { state: "done", passed: false }), "run.notPassed");
});

test("a state the API added after this page shows the fallback text", () => {
    const newer = { state: "cancelled" } as unknown as Parameters<typeof runText>[1];
    assert.equal(runText(t, newer), "run.unknown");
});

test("each case verdict has its own text", () => {
    assert.equal(caseText(t, "passed"), "case.passed");
    assert.equal(caseText(t, "wrong-answer"), "case.wrongAnswer");
    assert.equal(caseText(t, "crashed"), "case.crashed");
    assert.equal(caseText(t, "timed-out"), "case.timedOut");
});

test("a case verdict the API added after this page shows the fallback text", () => {
    const newer = "memory-exceeded" as unknown as Parameters<typeof caseText>[1];
    assert.equal(caseText(t, newer), "case.unknown");
});

test("a draft is saved, being saved, or not saved whatever the reason", () => {
    assert.equal(saveText(t, { kind: "saved" }), "draft.saved");
    assert.equal(saveText(t, { kind: "pending" }), "draft.saving");
    assert.equal(saveText(t, { kind: "failed", failure: { kind: "unreachable" } }), "draft.notSaved");
    assert.equal(saveText(t, { kind: "conflict", current: { revision: 2, source: "" } }), "draft.notSaved");
});

const at = "2026-01-01T09:00:00Z";
const now = new Date("2026-01-01T08:00:00Z");

test("an open exercise shows when it closes, or nothing when it never does", () => {
    assert.equal(availabilityText(t, { id: "sum", open: true, opens_at: null, closes_at: at }, now), `catalog.until {"at":${JSON.stringify(new Date(at))}}`);
    assert.equal(availabilityText(t, { id: "sum", open: true, opens_at: null, closes_at: null }, now), "");
});

test("a locked exercise says when it opens, that it closed, or that it has no date", () => {
    assert.equal(availabilityText(t, { id: "sum", open: false, opens_at: at, closes_at: null }, now), `catalog.opensOn {"at":${JSON.stringify(new Date(at))}}`);
    assert.equal(availabilityText(t, { id: "sum", open: false, opens_at: "2025-01-01T00:00:00Z", closes_at: null }, now), "catalog.closed");
    assert.equal(availabilityText(t, { id: "sum", open: false, opens_at: null, closes_at: null }, now), "catalog.notScheduled");
});
