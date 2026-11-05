// The API's routes, typed by the Rust contracts (packages/contracts, written by ts-rs).
// Nothing here throws: each call returns what happened, so the page decides what the student sees
// for every failure instead of letting one escape as an unhandled rejection.
import type { CatalogEntry } from "@log791/contracts/CatalogEntry";
import type { Credentials } from "@log791/contracts/Credentials";
import type { Id } from "@log791/contracts/Id";
import type { Identity } from "@log791/contracts/Identity";
import type { Run } from "@log791/contracts/Run";
import type { RunStatus } from "@log791/contracts/RunStatus";
import type { Submission } from "@log791/contracts/Submission";
import type { Submitted } from "@log791/contracts/Submitted";

// Same origin as the page: nginx sends /api/ to the API, so the session cookie goes with every call
const BASE = "/api";

export type Failure =
    /** The API answered an error: 401 when the session is over, 502 while it restarts behind nginx. */
    | { kind: "status"; status: number }
    /** No answer at all: the network is down, or the answer broke off. */
    | { kind: "unreachable" };

export type Outcome<T> = { ok: true; value: T } | { ok: false; failure: Failure };

// The body is the API's own JSON, typed by its contracts and kept compatible across one release
// (tests/contracts/compatible.sh): trusted as is rather than checked field by field.
const json = <T>(response: Response) => response.json() as Promise<T>;
const text = (response: Response) => response.text();
const nothing = async () => undefined;

async function call<T>(
    method: string,
    path: string,
    read: (response: Response) => Promise<T>,
    body?: unknown,
): Promise<Outcome<T>> {
    try {
        const response = await fetch(BASE + path, {
            method,
            ...(body === undefined ? {} : { headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }),
        });
        if (!response.ok) return { ok: false, failure: { kind: "status", status: response.status } };
        return { ok: true, value: await read(response) };
    } catch {
        return { ok: false, failure: { kind: "unreachable" } };
    }
}

const exercise = (course: Id, id: Id) =>
    `/courses/${encodeURIComponent(course)}/exercises/${encodeURIComponent(id)}`;

export const signIn = (credentials: Credentials) => call("POST", "/sessions", json<Identity>, credentials);
export const signOut = () => call("DELETE", "/sessions", nothing);

export const catalog = (course: Id) =>
    call("GET", `/courses/${encodeURIComponent(course)}/exercises`, json<CatalogEntry[]>);
export const statement = (course: Id, id: Id) => call("GET", `${exercise(course, id)}/statement`, text);
/** The student's own runs on an exercise, newest first */
export const history = (course: Id, id: Id) => call("GET", `${exercise(course, id)}/submissions`, json<Run[]>);

export const submit = (submission: Submission) => call("POST", "/submissions", json<Submitted>, submission);
export const progress = (run: string) => call("GET", `/submissions/${encodeURIComponent(run)}`, json<RunStatus>);
