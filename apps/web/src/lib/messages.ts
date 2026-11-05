// What the student reads for each outcome: the decisions of the page, apart from its markup.
import type { CatalogEntry } from "@log791/contracts/CatalogEntry";
import type { RunStatus } from "@log791/contracts/RunStatus";
import type { Failure } from "@log791/ui/api";
import type { SaveState } from "./draft.ts";
import type { Key, Translate } from "./i18n.ts";

/** The message for a failed call: the one `known` gives its status, else a general one. */
export function failureText(t: Translate, failure: Failure, known: Partial<Record<number, Key>> = {}): string {
    if (failure.kind === "unreachable") return t("server.unreachable");
    // 502: nginx answers for an API that is starting, as after an update
    const key = known[failure.status] ?? (failure.status === 502 ? "server.starting" : undefined);
    return key ? t(key) : t("server.error", { status: failure.status });
}

export const isSignedOut = (failure: Failure) => failure.kind === "status" && failure.status === 401;

/** A run's status in words. A state added to the API after this page shows the fallback (spec D22). */
export function runText(t: Translate, status: RunStatus): string {
    switch (status.state) {
        case "waiting":
            return t("run.waiting");
        case "running":
            return t("run.running");
        case "done":
            return t(status.passed ? "run.passed" : "run.notPassed");
        case "replaced":
            return t("run.replaced");
        case "failed":
            return t("run.failed");
        default:
            return t("run.unknown");
    }
}

/** Whether the draft is saved, beside the editor. Why it is not goes to the status line. */
export function saveText(t: Translate, state: SaveState): string {
    switch (state.kind) {
        case "saved":
            return t("draft.saved");
        case "pending":
            return t("draft.saving");
        default:
            return t("draft.notSaved");
    }
}

/** The note beside an exercise in the catalog. Whether it is open is the server's call (ADR-0020). */
export function availabilityText(t: Translate, entry: CatalogEntry, now: Date): string {
    if (entry.open) return entry.closes_at ? t("catalog.until", { at: new Date(entry.closes_at) }) : "";
    if (!entry.opens_at) return t("catalog.notScheduled");
    const opens = new Date(entry.opens_at);
    return opens > now ? t("catalog.opensOn", { at: opens }) : t("catalog.closed");
}
