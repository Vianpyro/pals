// The student's runs as the API streams them (apps/api/src/events.rs): on every connection a
// snapshot, the latest run on each exercise and the visible cases it reported so far, then each
// change. The browser reconnects by itself when the network drops or the server ends the stream,
// as it does every few minutes; the next snapshot fills in whatever changed meanwhile.
import type { CaseResult } from "@log791/contracts/CaseResult";
import type { Run } from "@log791/contracts/Run";

/** The part of EventSource used here, so a test can stand in for it. */
export type Source = {
    readonly readyState: number;
    addEventListener(type: string, listener: (event: Event) => void): void;
    close(): void;
};

/**
 * `onRun` for every run the stream sends, `onCase` for every visible case, possibly one already
 * received (ADR-0030), and `onClosed` once if the stream stops for good: the session was refused,
 * or the API has no stream. The page then polls instead (ADR-0008).
 */
export type Listeners = {
    onRun(run: Run): void;
    onCase(result: CaseResult): void;
    onClosed(): void;
};

// EventSource.CLOSED, which Node has no EventSource to read from
const CLOSED = 2;

// The API's own JSON, typed by its contracts (api.ts)
const data = <T>(event: Event): T => JSON.parse((event as MessageEvent<string>).data) as T;

/** Follows the stream. Returns what closes it, when the student signs out. */
export function followRuns(
    { onRun, onCase, onClosed }: Listeners,
    open: (url: string) => Source = (url) => new EventSource(url),
): { close: () => void } {
    const source = open("/api/events");
    source.addEventListener("run", (event) => onRun(data<Run>(event)));
    source.addEventListener("case", (event) => onCase(data<CaseResult>(event)));
    // While the browser retries, readyState is CONNECTING; CLOSED means it gave up
    source.addEventListener("error", () => {
        if (source.readyState === CLOSED) onClosed();
    });
    return { close: () => source.close() };
}
