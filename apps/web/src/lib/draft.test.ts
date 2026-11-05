import type { Draft } from "@log791/contracts/Draft";
import type { DraftWrite } from "@log791/contracts/DraftWrite";
import type { DraftSave, Outcome } from "@log791/ui/api";
import assert from "node:assert/strict";
import { afterEach, beforeEach, mock, test } from "node:test";
import { Autosave, IDLE_MS, MAX_MS, type SaveState } from "./draft.ts";

beforeEach(() => mock.timers.enable({ apis: ["setTimeout"] }));
afterEach(() => mock.timers.reset());

// Lets what the timers started run to its end; setImmediate is not mocked
const settle = () => new Promise((resolve) => setImmediate(resolve));

const applied = (write: DraftWrite): Outcome<DraftSave> => ({ ok: true, value: { kind: "saved", revision: write.base + 1 } });
const conflict = (current: Draft): Outcome<DraftSave> => ({ ok: true, value: { kind: "conflict", current } });

// The API, applying every write unless `answer` says otherwise; keeps the writes and the states shown
function server(answer: (write: DraftWrite) => Outcome<DraftSave> | Promise<Outcome<DraftSave>> = applied) {
    const writes: DraftWrite[] = [];
    const states: SaveState[] = [];
    return {
        writes,
        last: () => states.at(-1),
        open: (start: Draft) =>
            new Autosave(
                start,
                async (write) => {
                    writes.push(write);
                    return answer(write);
                },
                (state) => states.push(state),
            ),
    };
}

test("a draft is sent 2 s after the last keystroke, on the revision it was opened at", async () => {
    const api = server();
    const draft = api.open({ revision: 3, source: "" });
    draft.edit("p");
    assert.deepEqual(api.last(), { kind: "pending" });
    mock.timers.tick(IDLE_MS - 1);
    draft.edit("pr");
    mock.timers.tick(IDLE_MS - 1);
    await settle();
    assert.deepEqual(api.writes, []);

    mock.timers.tick(1);
    await settle();
    assert.deepEqual(api.writes, [{ base: 3, source: "pr" }]);
    assert.deepEqual(api.last(), { kind: "saved" });
});

test("typing that never pauses is still sent every 10 s", async () => {
    const api = server();
    const draft = api.open({ revision: 0, source: "" });
    for (let second = 1; second <= MAX_MS / 1000; second++) {
        draft.edit("x".repeat(second));
        mock.timers.tick(1000);
    }
    await settle();
    assert.deepEqual(api.writes, [{ base: 0, source: "x".repeat(MAX_MS / 1000) }]);
});

test("each write is based on the revision the previous one became", async () => {
    const api = server();
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    mock.timers.tick(IDLE_MS);
    await settle();
    draft.edit("ab");
    mock.timers.tick(IDLE_MS);
    await settle();
    assert.deepEqual(api.writes, [
        { base: 0, source: "a" },
        { base: 1, source: "ab" },
    ]);
});

test("coming back to the saved code sends nothing", async () => {
    const api = server();
    const draft = api.open({ revision: 1, source: "a" });
    draft.edit("ab");
    draft.edit("a");
    mock.timers.tick(MAX_MS);
    await settle();
    assert.deepEqual(api.writes, []);
    assert.deepEqual(api.last(), { kind: "saved" });
});

test("one write at a time: a flush waits for the write in flight, then sends on its revision", async () => {
    let release = () => {};
    const api = server(async (write) => {
        if (write.base === 0) await new Promise<void>((resolve) => (release = resolve));
        return applied(write);
    });
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    const first = draft.flush();
    draft.edit("ab");
    const second = draft.flush();
    await settle();
    assert.deepEqual(api.writes, [{ base: 0, source: "a" }]);

    release();
    await Promise.all([first, second]);
    assert.deepEqual(api.writes, [
        { base: 0, source: "a" },
        { base: 1, source: "ab" },
    ]);
    assert.deepEqual(api.last(), { kind: "saved" });
});

test("typing during a write is sent after the next pause", async () => {
    let release = () => {};
    const api = server(async (write) => {
        if (write.base === 0) await new Promise<void>((resolve) => (release = resolve));
        return applied(write);
    });
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    const first = draft.flush();
    draft.edit("ab");
    release();
    await first;
    assert.deepEqual(api.last(), { kind: "pending" });

    mock.timers.tick(IDLE_MS);
    await settle();
    assert.deepEqual(api.writes.at(-1), { base: 1, source: "ab" });
    assert.deepEqual(api.last(), { kind: "saved" });
});

const theirs = { revision: 2, source: "theirs" };

test("a stale revision stops saving until the student chooses", async () => {
    const api = server((write) => (write.base === 0 ? conflict(theirs) : applied(write)));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("mine");
    await draft.flush();
    assert.deepEqual(api.last(), { kind: "conflict", current: theirs });

    draft.edit("mine, still typing");
    mock.timers.tick(MAX_MS);
    await draft.flush();
    assert.equal(api.writes.length, 1);
    assert.deepEqual(api.last(), { kind: "conflict", current: theirs });
});

test("keeping one's version writes it over the current revision", async () => {
    const api = server((write) => (write.base === 0 ? conflict(theirs) : applied(write)));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("mine");
    await draft.flush();
    draft.edit("mine, still typing");

    await draft.keepMine();
    assert.deepEqual(api.writes.at(-1), { base: 2, source: "mine, still typing" });
    assert.deepEqual(api.last(), { kind: "saved" });
});

test("taking the saved version gives it back, and writing goes on from its revision", async () => {
    const api = server((write) => (write.base === 0 ? conflict(theirs) : applied(write)));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("mine");
    await draft.flush();

    assert.equal(draft.takeTheirs(), "theirs");
    assert.deepEqual(api.last(), { kind: "saved" });
    assert.equal(api.writes.length, 1);
    draft.edit("theirs, edited");
    await draft.flush();
    assert.deepEqual(api.writes.at(-1), { base: 2, source: "theirs, edited" });
});

test("choosing a version when there is no conflict changes nothing", async () => {
    const api = server();
    const draft = api.open({ revision: 1, source: "a" });
    draft.edit("ab");
    assert.equal(draft.takeTheirs(), "ab");
    await draft.keepMine();
    assert.deepEqual(api.writes, []);
    assert.deepEqual(api.last(), { kind: "pending" });
});

test("a server out of reach is tried again 10 s later", async () => {
    let reachable = false;
    const api = server((write) => (reachable ? applied(write) : { ok: false, failure: { kind: "unreachable" } }));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    await draft.flush();
    assert.deepEqual(api.last(), { kind: "failed", failure: { kind: "unreachable" } });

    reachable = true;
    mock.timers.tick(MAX_MS - 1);
    await settle();
    assert.equal(api.writes.length, 1);
    mock.timers.tick(1);
    await settle();
    assert.deepEqual(api.writes.at(-1), { base: 0, source: "a" });
    assert.deepEqual(api.last(), { kind: "saved" });
});

test("a server error is tried again too", async () => {
    const api = server(() => ({ ok: false, failure: { kind: "status", status: 500 } }));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    await draft.flush();
    mock.timers.tick(MAX_MS);
    await settle();
    assert.equal(api.writes.length, 2);
});

test("typing after a failure keeps showing it until a write succeeds", async () => {
    const api = server(() => ({ ok: false, failure: { kind: "status", status: 502 } }));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    await draft.flush();
    draft.edit("ab");
    assert.deepEqual(api.last(), { kind: "failed", failure: { kind: "status", status: 502 } });
});

test("a refusal is not tried again by itself", async () => {
    const api = server(() => ({ ok: false, failure: { kind: "status", status: 403 } }));
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    await draft.flush();
    mock.timers.tick(3 * MAX_MS);
    await settle();
    assert.equal(api.writes.length, 1);
});

test("closing sends what was typed at once, then nothing more", async () => {
    const api = server();
    const draft = api.open({ revision: 0, source: "" });
    draft.edit("a");
    await draft.close();
    assert.deepEqual(api.writes, [{ base: 0, source: "a" }]);

    draft.edit("ab");
    mock.timers.tick(MAX_MS);
    await settle();
    assert.equal(api.writes.length, 1);
});
