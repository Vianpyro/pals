import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { catalog, progress, saveDraft, signOut, submit } from "./api.ts";

type Sent = { url: string; method: string; headers: Headers; body: string | null };

const realFetch = globalThis.fetch;
afterEach(() => {
    globalThis.fetch = realFetch;
});

// The API answers `response`, or fetch rejects as when the network is down; returns what was sent
function serve(response: Response | "unreachable"): Sent[] {
    const sent: Sent[] = [];
    globalThis.fetch = async (input, init) => {
        sent.push({
            url: String(input),
            method: init?.method ?? "GET",
            headers: new Headers(init?.headers),
            body: typeof init?.body === "string" ? init.body : null,
        });
        if (response === "unreachable") throw new TypeError("Failed to fetch");
        return response;
    };
    return sent;
}

test("reads a course's catalog, its identifier kept inside one path segment", async () => {
    const entries = [{ id: "sum", open: true, opens_at: null, closes_at: null }];
    const sent = serve(Response.json(entries));
    assert.deepEqual(await catalog("log/200"), { ok: true, value: entries });
    assert.equal(sent[0].url, "/api/courses/log%2F200/exercises");
});

test("an error status is a failure carrying it", async () => {
    serve(new Response(null, { status: 401 }));
    assert.deepEqual(await progress("0193"), { ok: false, failure: { kind: "status", status: 401 } });
});

test("a server out of reach is a failure, not an exception", async () => {
    serve("unreachable");
    assert.deepEqual(await catalog("log200"), { ok: false, failure: { kind: "unreachable" } });
});

test("a submission is posted as JSON and answers the new run's id", async () => {
    const sent = serve(Response.json({ id: "0193" }, { status: 202 }));
    const submission = { course: "log200", exercise: "sum", language_pack: "python", source: "print(1)" };
    assert.deepEqual(await submit(submission), { ok: true, value: { id: "0193" } });
    assert.equal(sent[0].method, "POST");
    assert.equal(sent[0].headers.get("Content-Type"), "application/json");
    assert.deepEqual(JSON.parse(sent[0].body ?? ""), submission);
});

test("signing out reads no body", async () => {
    serve(new Response(null, { status: 204 }));
    assert.deepEqual(await signOut(), { ok: true, value: undefined });
});

test("a draft is put as JSON on its exercise and answers the revision it became", async () => {
    const sent = serve(Response.json({ revision: 4 }));
    assert.deepEqual(await saveDraft("log200", "sum", { base: 3, source: "print(1)" }), {
        ok: true,
        value: { kind: "saved", revision: 4 },
    });
    assert.equal(sent[0].method, "PUT");
    assert.equal(sent[0].url, "/api/courses/log200/exercises/sum/draft");
    assert.deepEqual(JSON.parse(sent[0].body ?? ""), { base: 3, source: "print(1)" });
});

test("a draft written on a stale revision answers the current one, not a failure", async () => {
    serve(Response.json({ revision: 5, source: "print(2)" }, { status: 409 }));
    assert.deepEqual(await saveDraft("log200", "sum", { base: 3, source: "print(1)" }), {
        ok: true,
        value: { kind: "conflict", current: { revision: 5, source: "print(2)" } },
    });
});

test("a 409 elsewhere stays a failure", async () => {
    serve(new Response(null, { status: 409 }));
    assert.deepEqual(await progress("0193"), { ok: false, failure: { kind: "status", status: 409 } });
});
