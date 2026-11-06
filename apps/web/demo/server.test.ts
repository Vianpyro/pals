import assert from "node:assert/strict";
import { test } from "node:test";
import type { Run } from "@log791/contracts/Run";
import type { Outcome as CaseOutcome } from "./judging.ts";
import { createServer, PASSWORD } from "./server.ts";
import { memoryStorage } from "./storage.ts";

const COURSE = "log200";
const sum = { id: "sum", statement: "# Somme", cases: [{ input: "1 2\n", expected: "3\n" }] };

type Run_ = (source: string, input: string) => Promise<CaseOutcome>;
const rightAnswer: Run_ = async () => ({ kind: "exited", success: true, stdout: "3\n" });

function server({ storage = memoryStorage(), run = rightAnswer } = {}) {
    let ids = 0;
    return createServer(storage, { course: COURSE, exercises: [sum] }, run, () => `run-${++ids}`);
}

async function signedIn(options = {}) {
    const created = server(options);
    await created.api.signIn({ id: "etudiant1", password: PASSWORD });
    return created;
}

const value = <T>(outcome: { ok: true; value: T } | { ok: false }): T => {
    assert.ok(outcome.ok);
    return outcome.value;
};
const refused = (status: number) => ({ ok: false, failure: { kind: "status", status } });
const submission = { course: COURSE, exercise: "sum", language_pack: "python", source: "print(3)" };

// The states a run goes through, until it is judged
function follow(created: ReturnType<typeof server>) {
    const seen: Run[] = [];
    const done = new Promise<void>((resolve) => {
        created.followRuns(
            (run) => {
                seen.push(run);
                if (run.state === "done" || run.state === "failed") resolve();
            },
            () => {},
        );
    });
    return { seen, done };
}

test("everything but signing in asks for a session", async () => {
    const { api } = server();
    assert.deepEqual(await api.catalog(COURSE), refused(401));
    assert.deepEqual(await api.statement(COURSE, "sum"), refused(401));
    assert.deepEqual(await api.history(COURSE, "sum"), refused(401));
    assert.deepEqual(await api.draft(COURSE, "sum"), refused(401));
    assert.deepEqual(await api.saveDraft(COURSE, "sum", { base: 0, source: "" }), refused(401));
    assert.deepEqual(await api.submit(submission), refused(401));
    assert.deepEqual(await api.progress("run-1"), refused(401));
});

test("signs in with the demo's password only, and out again", async () => {
    const { api } = server();
    assert.deepEqual(await api.signIn({ id: "etudiant1", password: "wrong" }), refused(401));
    assert.deepEqual(value(await api.signIn({ id: "etudiant1", password: PASSWORD })), {
        id: "etudiant1",
        name: null,
        email: null,
    });
    assert.ok((await api.catalog(COURSE)).ok);

    assert.ok((await api.signOut()).ok);
    assert.deepEqual(await api.catalog(COURSE), refused(401));
});

test("never keeps the password", async () => {
    const storage = memoryStorage();
    const written: string[] = [];
    const { setItem } = storage;
    storage.setItem = (key, data) => {
        written.push(key + data);
        setItem(key, data);
    };
    await server({ storage }).api.signIn({ id: "etudiant1", password: PASSWORD });
    assert.equal(written.filter((entry) => entry.includes(PASSWORD)).length, 0);
});

test("the session survives a reload", async () => {
    const storage = memoryStorage();
    await signedIn({ storage });
    assert.ok((await server({ storage }).api.catalog(COURSE)).ok);
});

test("lists the course's exercises, all open, and shows their statements", async () => {
    const { api } = await signedIn();
    assert.deepEqual(value(await api.catalog(COURSE)), [{ id: "sum", open: true, opens_at: null, closes_at: null }]);
    assert.equal(value(await api.statement(COURSE, "sum")), "# Somme");
});

test("an unknown course or exercise is not found", async () => {
    const { api } = await signedIn();
    assert.deepEqual(await api.catalog("other"), refused(404));
    assert.deepEqual(await api.statement(COURSE, "constructor"), refused(404));
    assert.deepEqual(await api.statement("other", "sum"), refused(404));
    assert.deepEqual(await api.submit({ ...submission, exercise: "missing" }), refused(404));
    assert.deepEqual(await api.progress("missing"), refused(404));
});

test("a draft starts empty at revision 0 and each write adds a revision", async () => {
    const { api } = await signedIn();
    assert.deepEqual(value(await api.draft(COURSE, "sum")), { revision: 0, source: "" });
    assert.deepEqual(value(await api.saveDraft(COURSE, "sum", { base: 0, source: "a" })), { kind: "saved", revision: 1 });
    assert.deepEqual(value(await api.saveDraft(COURSE, "sum", { base: 1, source: "b" })), { kind: "saved", revision: 2 });
    assert.deepEqual(value(await api.draft(COURSE, "sum")), { revision: 2, source: "b" });
});

test("a write based on an older revision is refused with the current draft, as from another tab", async () => {
    const storage = memoryStorage();
    const tab = await signedIn({ storage });
    const other = server({ storage });
    await other.api.saveDraft(COURSE, "sum", { base: 0, source: "theirs" });

    const save = value(await tab.api.saveDraft(COURSE, "sum", { base: 0, source: "mine" }));
    assert.deepEqual(save, { kind: "conflict", current: { revision: 1, source: "theirs" } });
    assert.deepEqual(value(await tab.api.draft(COURSE, "sum")), { revision: 1, source: "theirs" });
});

test("a draft the browser has no room for is refused, not lost silently", async () => {
    const storage = memoryStorage();
    const { api } = await signedIn({ storage });
    storage.setItem = () => {
        throw new DOMException("full", "QuotaExceededError");
    };
    assert.deepEqual(await api.saveDraft(COURSE, "sum", { base: 0, source: "a" }), refused(422));
});

test("each student has their own drafts and runs", async () => {
    const storage = memoryStorage();
    const first = await signedIn({ storage });
    await first.api.saveDraft(COURSE, "sum", { base: 0, source: "mine" });
    const followed = follow(first);
    await first.api.submit(submission);
    await followed.done;

    await first.api.signIn({ id: "etudiant2", password: PASSWORD });
    assert.deepEqual(value(await first.api.draft(COURSE, "sum")), { revision: 0, source: "" });
    assert.deepEqual(value(await first.api.history(COURSE, "sum")), []);
});

test("a run goes from waiting through running to its verdict, in the stream and the history", async () => {
    const created = await signedIn();
    const { seen, done } = follow(created);

    const { id } = value(await created.api.submit(submission));
    await done;

    assert.deepEqual(
        seen.map(({ id: run, state }) => [run, state]),
        [
            [id, "waiting"],
            [id, "running"],
            [id, "done"],
        ],
    );
    assert.deepEqual(seen.at(-1), { ...seen.at(-1), state: "done", passed: true });
    const [listed] = value(await created.api.history(COURSE, "sum"));
    assert.equal(listed.state, "done");
    assert.equal(value(await created.api.progress(id)).state, "done");
});

test("a wrong program is judged as not passed", async () => {
    const created = await signedIn({ run: async () => ({ kind: "exited", success: true, stdout: "0\n" }) });
    const { seen, done } = follow(created);
    await created.api.submit(submission);
    await done;
    assert.deepEqual(seen.at(-1), { ...seen.at(-1), state: "done", passed: false });
});

test("a runtime that cannot run the program fails the run", async () => {
    const created = await signedIn({
        run: async () => {
            throw new Error("Pyodide did not load");
        },
    });
    const { seen, done } = follow(created);
    await created.api.submit(submission);
    await done;
    assert.equal(seen.at(-1)?.state, "failed");
});

test("the program and its input reach the runtime", async () => {
    const ran: [string, string][] = [];
    const created = await signedIn({
        run: async (source: string, input: string) => {
            ran.push([source, input]);
            return { kind: "exited", success: true, stdout: "3\n" } as const;
        },
    });
    const { done } = follow(created);
    await created.api.submit(submission);
    await done;
    assert.deepEqual(ran, [["print(3)", "1 2\n"]]);
});

test("runs are judged one at a time, newest first in the history", async () => {
    let running = 0;
    let most = 0;
    const created = await signedIn({
        run: async () => {
            most = Math.max(most, ++running);
            await new Promise((resolve) => setTimeout(resolve, 1));
            running--;
            return { kind: "exited", success: true, stdout: "3\n" } as const;
        },
    });
    const first = value(await created.api.submit(submission)).id;
    const second = value(await created.api.submit(submission)).id;
    const { done } = follow(created);
    assert.deepEqual(
        value(await created.api.history(COURSE, "sum")).map((run) => run.id),
        [second, first],
    );
    await done;
    await new Promise((resolve) => setTimeout(resolve, 10));
    assert.equal(most, 1);
});

test("a run cut off by a reload shows as failed, not as waiting forever", async () => {
    const storage = memoryStorage();
    const before = await signedIn({ storage, run: () => new Promise(() => {}) });
    const { id } = value(await before.api.submit(submission));

    const after = server({ storage });
    assert.equal(value(await after.api.history(COURSE, "sum"))[0].state, "failed");
    assert.equal(value(await after.api.progress(id)).state, "failed");
});

test("a closed stream hears nothing more", async () => {
    const created = await signedIn();
    const seen: Run[] = [];
    created.followRuns((run) => seen.push(run), () => {}).close();
    const { done } = follow(created);
    await created.api.submit(submission);
    await done;
    assert.deepEqual(seen, []);
});

test("keeps a bounded history", async () => {
    const created = await signedIn();
    const { done } = follow(created);
    for (let index = 0; index < 60; index++) await created.api.submit(submission);
    await done;
    assert.equal(value(await created.api.history(COURSE, "sum")).length, 50);
});

test("a stored value that is not JSON reads as absent", async () => {
    const storage = memoryStorage();
    storage.setItem("pals-demo:session", "not json");
    assert.deepEqual(await server({ storage }).api.catalog(COURSE), refused(401));
});
