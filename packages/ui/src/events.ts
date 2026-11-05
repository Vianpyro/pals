// The student's runs as the API streams them (apps/api/src/events.rs): on every connection a
// snapshot, the latest run on each exercise, then each change. The browser reconnects by itself
// when the network drops or the server ends the stream, as it does every few minutes; the next
// snapshot fills in whatever changed meanwhile.
import type { Run } from "@log791/contracts/Run";

/** The part of EventSource used here, so a test can stand in for it. */
export type Source = {
    readonly readyState: number;
    addEventListener(type: string, listener: (event: Event) => void): void;
    close(): void;
};

// EventSource.CLOSED, which Node has no EventSource to read from
const CLOSED = 2;

/**
 * Calls `onRun` for every run the stream sends, and `onClosed` once if it stops for good: the
 * session was refused, or the API has no stream. The page then polls instead (ADR-0008).
 * Returns what closes the stream, when the student signs out.
 */
export function followRuns(
    onRun: (run: Run) => void,
    onClosed: () => void,
    open: (url: string) => Source = (url) => new EventSource(url),
): { close: () => void } {
    const source = open("/api/events");
    // The API's own JSON, typed by its contracts (api.ts)
    source.addEventListener("run", (event) => onRun(JSON.parse((event as MessageEvent<string>).data) as Run));
    // While the browser retries, readyState is CONNECTING; CLOSED means it gave up
    source.addEventListener("error", () => {
        if (source.readyState === CLOSED) onClosed();
    });
    return { close: () => source.close() };
}
