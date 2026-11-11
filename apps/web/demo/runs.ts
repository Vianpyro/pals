// The students' runs: kept with no source, judged one at a time, and told to whoever follows them.
import type { Run } from "@log791/contracts/Run";
import type { RunStatus } from "@log791/contracts/RunStatus";
import type { Listeners } from "@log791/ui/events";
import { judgeCases, type Case, type Outcome, type Report } from "./judging.ts";
import type { Records } from "./records.ts";

const MAX_RUNS = 50;

type Owner = { student: string; offering: string; exercise: string };
type StoredRun = Owner & { run: Run };

export type RunCase = (source: string, input: string) => Promise<Outcome>;

const unjudged = ({ state }: Run) => state === "waiting" || state === "running";
const sameOwner = (a: Owner, b: Owner) => a.student === b.student && a.offering === b.offering && a.exercise === b.exercise;

export function createRuns(records: Records, runCase: RunCase, newId: () => string) {
    const followers = new Set<Listeners>();
    // The runs this tab is judging. A worker dies with its tab, so no other unjudged run will finish.
    const judging = new Set<string>();
    let queue = Promise.resolve();

    const stored = () => records.read<StoredRun[]>("runs") ?? [];

    function current({ run }: StoredRun): Run {
        return unjudged(run) && !judging.has(run.id) ? { id: run.id, kind: run.kind, enqueued_at: run.enqueued_at, state: "failed" } : run;
    }

    function publish(next: StoredRun) {
        // shortcut: a run reaches only the tab that submitted it; another tab sees it when it next reads the history
        records.write("runs", [next, ...stored().filter(({ run }) => run.id !== next.run.id)].slice(0, MAX_RUNS));
        for (const follower of followers) follower.onRun(next.run);
    }

    // shortcut: cases are not stored, so a reload shows none; the API replays the latest run's (ADR-0030)
    function tell(run: string, report: Report) {
        for (const follower of followers) follower.onCase({ run, ...report });
    }

    function advance(from: StoredRun, status: RunStatus): StoredRun {
        const next = { ...from, run: { id: from.run.id, kind: from.run.kind, enqueued_at: from.run.enqueued_at, ...status } };
        publish(next);
        return next;
    }

    async function judge(waiting: StoredRun, cases: readonly Case[], source: string) {
        const running = advance(waiting, { state: "running" });
        let status: RunStatus;
        try {
            const passed = await judgeCases(
                cases,
                (input) => runCase(source, input),
                (report) => tell(running.run.id, report),
            );
            status = { state: "done", passed };
        } catch {
            status = { state: "failed" };
        }
        judging.delete(running.run.id);
        advance(running, status);
    }

    return {
        /** Queues a run and returns its id. One worker, one program at a time; judge() catches what a run can throw. */
        start(owner: Owner, cases: readonly Case[], source: string): string {
            const run: Run = { id: newId(), kind: "check", enqueued_at: new Date().toISOString(), state: "waiting" };
            const waiting = { ...owner, run };
            judging.add(run.id);
            publish(waiting);
            queue = queue.then(() => judge(waiting, cases, source));
            return run.id;
        },

        /** A student's runs on an exercise, newest first. */
        of: (owner: Owner) =>
            stored()
                .filter((entry) => sameOwner(entry, owner))
                .map(current),

        find(student: string, id: string): Run | undefined {
            const found = stored().find((entry) => entry.student === student && entry.run.id === id);
            return found && current(found);
        },

        follow(listeners: Listeners) {
            followers.add(listeners);
            return { close: () => void followers.delete(listeners) };
        },
    };
}
