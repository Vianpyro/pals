// Runs Python programs in a worker, which is the only way to stop an endless one: terminate() it.
import type { Outcome } from "./judging.ts";

/** What python.worker.ts says: once when it starts, then once per program. */
export type WorkerMessage = { kind: "ready" } | { kind: "load-failed" } | { kind: "broken" } | Extract<Outcome, { kind: "exited" }>;

/** The part of Worker used here, so a test can stand in for it. */
export type PythonWorker = {
    onmessage: ((event: MessageEvent<WorkerMessage>) => void) | null;
    onerror: ((event: ErrorEvent) => void) | null;
    postMessage(message: unknown): void;
    terminate(): void;
};

const CRASHED: Outcome = { kind: "exited", success: false, stdout: "" };

export function createPython(spawn: () => PythonWorker, limits: { runMs: number; loadMs: number }) {
    let started: Promise<PythonWorker> | null = null;

    // A new worker for the next program: a stopped or broken one cannot run anything more
    function discard(worker: PythonWorker) {
        worker.terminate();
        started = null;
    }

    function start(): Promise<PythonWorker> {
        return new Promise((resolve, reject) => {
            const worker = spawn();
            const fail = () => {
                clearTimeout(timer);
                discard(worker);
                reject(new Error("Pyodide did not load"));
            };
            const timer = setTimeout(fail, limits.loadMs);
            worker.onerror = fail;
            worker.onmessage = ({ data }) => {
                if (data.kind !== "ready") return fail();
                clearTimeout(timer);
                resolve(worker);
            };
        });
    }

    return async function run(source: string, input: string): Promise<Outcome> {
        const worker = await (started ??= start());
        return new Promise((resolve) => {
            const finish = (outcome: Outcome, keep: boolean) => {
                clearTimeout(timer);
                if (!keep) discard(worker);
                resolve(outcome);
            };
            // From here, not from the worker's start: a slow download is not the program's fault
            const timer = setTimeout(() => finish({ kind: "timed-out" }, false), limits.runMs);
            worker.onerror = () => finish(CRASHED, false);
            worker.onmessage = ({ data }) =>
                data.kind === "exited" ? finish(data, true) : finish(CRASHED, false);
            worker.postMessage({ source, input });
        });
    };
}
