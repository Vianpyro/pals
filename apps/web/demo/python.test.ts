import assert from "node:assert/strict";
import { test } from "node:test";
import { createPython, type PythonWorker, type WorkerMessage } from "./python.ts";

const exited = { kind: "exited", success: true, stdout: "3\n" } as const;
const limits = { runMs: 5, loadMs: 50 };

// A worker the test drives: `behave` says what it does once started and for each program
class FakeWorker implements PythonWorker {
    onmessage: PythonWorker["onmessage"] = null;
    onerror: PythonWorker["onerror"] = null;
    posted: unknown[] = [];
    terminated = false;
    #onProgram: (worker: FakeWorker) => void;

    constructor(onStart: (worker: FakeWorker) => void, onProgram: (worker: FakeWorker) => void) {
        this.#onProgram = onProgram;
        queueMicrotask(() => onStart(this));
    }
    postMessage(message: unknown) {
        this.posted.push(message);
        queueMicrotask(() => this.#onProgram(this));
    }
    terminate() {
        this.terminated = true;
    }
    say(data: WorkerMessage) {
        this.onmessage?.(new MessageEvent("message", { data }));
    }
    fail() {
        this.onerror?.({} as ErrorEvent);
    }
}

const ready = (worker: FakeWorker) => worker.say({ kind: "ready" });
const silent = () => {};

function spawner(onProgram: (worker: FakeWorker) => void = (worker) => worker.say(exited), onStart = ready) {
    const workers: FakeWorker[] = [];
    const spawn = () => {
        const worker = new FakeWorker(onStart, onProgram);
        workers.push(worker);
        return worker;
    };
    return { workers, spawn };
}

test("sends the program and its input once the runtime is ready, and returns what it did", async () => {
    const { workers, spawn } = spawner();
    assert.deepEqual(await createPython(spawn, limits)("print(3)", "1 2\n"), exited);
    assert.deepEqual(workers[0].posted, [{ source: "print(3)", input: "1 2\n" }]);
});

test("loads the runtime once for several programs", async () => {
    const { workers, spawn } = spawner();
    const run = createPython(spawn, limits);
    await run("a", "");
    await run("b", "");
    assert.equal(workers.length, 1);
    assert.equal(workers[0].terminated, false);
});

test("the time limit covers the program, not the runtime's loading", async () => {
    const { spawn } = spawner(undefined, (worker) => {
        setTimeout(() => ready(worker), limits.runMs * 4);
    });
    assert.deepEqual(await createPython(spawn, limits)("a", ""), exited);
});

test("an endless program times out, and the next one gets a new worker", async () => {
    const { workers, spawn } = spawner(silent);
    const run = createPython(spawn, limits);
    assert.deepEqual(await run("while True: pass", ""), { kind: "timed-out" });
    assert.equal(workers[0].terminated, true);

    assert.deepEqual(await run("again", ""), { kind: "timed-out" });
    assert.equal(workers.length, 2);
});

test("a broken runtime counts as a crash, and the next program gets a new worker", async () => {
    const { workers, spawn } = spawner((worker) => worker.say({ kind: "broken" }));
    const run = createPython(spawn, limits);
    assert.deepEqual(await run("a", ""), { kind: "exited", success: false, stdout: "" });
    assert.equal(workers[0].terminated, true);
    await run("b", "");
    assert.equal(workers.length, 2);
});

test("a worker error while running counts as a crash, and the next program gets a new worker", async () => {
    const { workers, spawn } = spawner((worker) => worker.fail());
    const run = createPython(spawn, limits);
    assert.deepEqual(await run("a", ""), { kind: "exited", success: false, stdout: "" });
    await run("b", "");
    assert.equal(workers.length, 2);
});

test("a runtime that cannot load rejects, and the next program tries again", async () => {
    const { workers, spawn } = spawner(silent, (worker) => worker.say({ kind: "load-failed" }));
    const run = createPython(spawn, limits);
    await assert.rejects(run("a", ""));
    assert.equal(workers[0].terminated, true);
    await assert.rejects(run("b", ""));
    assert.equal(workers.length, 2);
});

test("a script that fails to start rejects", async () => {
    const { spawn } = spawner(silent, (worker) => worker.fail());
    await assert.rejects(createPython(spawn, limits)("a", ""));
});

test("a runtime that never answers stops being waited for", async () => {
    const { workers, spawn } = spawner(silent, silent);
    await assert.rejects(createPython(spawn, { runMs: 5, loadMs: 1 })("a", ""));
    assert.equal(workers[0].terminated, true);
});
