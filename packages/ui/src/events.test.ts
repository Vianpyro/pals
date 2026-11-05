import assert from "node:assert/strict";
import { test } from "node:test";
import { followRuns, type Source } from "./events.ts";

// An EventSource the test drives: it sends events and sets the connection state
class FakeSource extends EventTarget implements Source {
    readyState = 1;
    closed = false;
    url = "";
    close() {
        this.closed = true;
    }
    send(type: string, data: unknown) {
        this.dispatchEvent(new MessageEvent(type, { data: JSON.stringify(data) }));
    }
}

function open() {
    const source = new FakeSource();
    return {
        source,
        open: (url: string) => {
            source.url = url;
            return source;
        },
    };
}

const run = { id: "0193", state: "done", passed: true, enqueued_at: "2026-01-01T09:00:00Z" } as const;

test("passes on every run the stream sends", () => {
    const { source, open: connect } = open();
    const received: unknown[] = [];
    followRuns((sent) => received.push(sent), () => {}, connect);
    source.send("run", run);
    assert.equal(source.url, "/api/events");
    assert.deepEqual(received, [run]);
});

test("a dropped connection the browser retries is not the end", () => {
    const { source, open: connect } = open();
    let closed = false;
    followRuns(() => {}, () => (closed = true), connect);
    source.readyState = 0; // CONNECTING: the browser retries by itself
    source.dispatchEvent(new Event("error"));
    assert.equal(closed, false);
});

test("a stream the browser gives up on is reported, so the page can poll", () => {
    const { source, open: connect } = open();
    let closed = false;
    followRuns(() => {}, () => (closed = true), connect);
    source.readyState = 2; // CLOSED: refused session, or an API without the stream
    source.dispatchEvent(new Event("error"));
    assert.equal(closed, true);
});

test("closing the stream closes the connection", () => {
    const { source, open: connect } = open();
    followRuns(() => {}, () => {}, connect).close();
    assert.equal(source.closed, true);
});
