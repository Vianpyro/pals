// Saving a student's draft while they type (ADR-0038). The server's copy is the truth, written
// revision after revision. One write at a time, so each write names the revision the previous one
// became; a write that names an older one comes back as a conflict, and nothing is saved until the
// student chooses a version.
import type { Draft } from "@log791/contracts/Draft";
import type { DraftWrite } from "@log791/contracts/DraftWrite";
import type { DraftSave, Failure, Outcome } from "@log791/ui/api";

/** A write goes 2 s after the last keystroke, and at the latest 10 s after the first unsent one (ADR-0038). */
export const IDLE_MS = 2000;
export const MAX_MS = 10_000;

export type SaveState =
    | { kind: "saved" }
    /** Typed and not yet acknowledged */
    | { kind: "pending" }
    | { kind: "failed"; failure: Failure }
    /** The server holds a newer revision: the student keeps theirs or takes this one */
    | { kind: "conflict"; current: Draft };

// Out of reach or restarting: worth another try. A refusal (closed exercise, too large) is not, until the next edit.
const retryable = (failure: Failure) => failure.kind === "unreachable" || failure.status >= 500;

export class Autosave {
    #base: number;
    // The source as of #base
    #saved: string;
    #source: string;
    #conflict: Draft | null = null;
    #failed = false;
    #sending: Promise<void> | null = null;
    #closed = false;
    #idle: ReturnType<typeof setTimeout> | undefined;
    #latest: ReturnType<typeof setTimeout> | undefined;
    #save: (write: DraftWrite) => Promise<Outcome<DraftSave>>;
    #onState: (state: SaveState) => void;

    constructor(start: Draft, save: (write: DraftWrite) => Promise<Outcome<DraftSave>>, onState: (state: SaveState) => void) {
        this.#base = start.revision;
        this.#saved = this.#source = start.source;
        this.#save = save;
        this.#onState = onState;
    }

    get #clean() {
        return this.#source === this.#saved;
    }

    edit(source: string) {
        this.#source = source;
        // A conflict or a failure stays shown until it is resolved
        if (!this.#conflict && !this.#failed) this.#onState({ kind: this.#clean ? "saved" : "pending" });
        if (this.#clean) this.#cancel();
        else this.#schedule();
    }

    /** Sends what is not saved yet, now */
    async flush() {
        this.#cancel();
        while (this.#sending) await this.#sending;
        if (this.#conflict || this.#clean) return;
        this.#sending = this.#send().finally(() => (this.#sending = null));
        await this.#sending;
    }

    keepMine() {
        if (!this.#conflict) return Promise.resolve();
        this.#base = this.#conflict.revision;
        this.#conflict = null;
        return this.flush();
    }

    /** Gives the server's version, which the editor shows from then on */
    takeTheirs() {
        if (!this.#conflict) return this.#source;
        this.#base = this.#conflict.revision;
        this.#saved = this.#source = this.#conflict.source;
        this.#conflict = null;
        this.#cancel();
        this.#onState({ kind: "saved" });
        return this.#source;
    }

    /** Sends what is not saved yet, then stops */
    async close() {
        await this.flush();
        this.#closed = true;
        this.#cancel();
    }

    #cancel() {
        clearTimeout(this.#idle);
        clearTimeout(this.#latest);
        this.#idle = this.#latest = undefined;
    }

    #later(ms: number) {
        return setTimeout(() => void this.flush(), ms);
    }

    #schedule() {
        if (this.#closed || this.#conflict || this.#clean) return;
        clearTimeout(this.#idle);
        this.#idle = this.#later(IDLE_MS);
        this.#latest ??= this.#later(MAX_MS);
    }

    async #send() {
        const write = { base: this.#base, source: this.#source };
        const outcome = await this.#save(write);
        if (!outcome.ok) return this.#fail(outcome.failure);
        this.#failed = false;
        if (outcome.value.kind === "conflict") {
            this.#conflict = outcome.value.current;
            return this.#onState({ kind: "conflict", current: this.#conflict });
        }
        this.#base = outcome.value.revision;
        this.#saved = write.source;
        // Typing that went on during the write already scheduled the next one
        this.#onState({ kind: this.#clean ? "saved" : "pending" });
    }

    #fail(failure: Failure) {
        this.#failed = true;
        this.#onState({ kind: "failed", failure });
        this.#cancel();
        // Not every 2 s: 250 students retrying would only slow down a restarting API
        if (retryable(failure) && !this.#closed) this.#latest = this.#later(MAX_MS);
    }
}
