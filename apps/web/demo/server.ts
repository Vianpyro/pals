// What the API does, in the browser: the demo has no server (GitHub Pages serves files).
// `typeof Api` below makes the type checker refuse any drift from the real client (packages/ui).
import type { Identity } from "@log791/contracts/Identity";
import type { Run } from "@log791/contracts/Run";
import type * as Api from "@log791/ui/api";
import type { Exercise } from "./content.ts";
import { readDraft, writeDraft } from "./drafts.ts";
import { createRecords } from "./records.ts";
import { createRuns, type RunCase } from "./runs.ts";
import type { KeyValues } from "./storage.ts";

/** Shown on the page: a demo account, never anyone's real password. */
export const PASSWORD = "log791-dev";

type Content = { course: string; exercises: readonly Exercise[] };

const ok = <T>(value: T): Api.Outcome<T> => ({ ok: true, value });
const refused = (status: number): Api.Outcome<never> => ({ ok: false, failure: { kind: "status", status } });

const findExercise = (content: Content, course: string, id: string) =>
    course === content.course ? content.exercises.find((known) => known.id === id) : undefined;

const draftKey = (student: string, course: string, id: string) => `draft:${student}:${course}:${id}`;

export function createServer(
    storage: KeyValues,
    content: Content,
    runCase: RunCase,
    newId: () => string = () => crypto.randomUUID(),
): { api: typeof Api; followRuns: (onRun: (run: Run) => void, onClosed: () => void) => { close: () => void } } {
    const records = createRecords(storage);
    const runs = createRuns(records, runCase, newId);

    function signedIn<T>(answer: (student: string) => Api.Outcome<T>): Api.Outcome<T> {
        const student = records.read<Identity>("session")?.id;
        return student === undefined ? refused(401) : answer(student);
    }

    /** Refuses as the API does: no session first, then no such exercise. */
    const gate = <T>(course: string, id: string, answer: (student: string, exercise: Exercise) => Api.Outcome<T>) =>
        signedIn((student) => {
            const exercise = findExercise(content, course, id);
            return exercise ? answer(student, exercise) : refused(404);
        });

    const api: typeof Api = {
        async signIn({ id, password }) {
            if (password !== PASSWORD) return refused(401);
            const identity: Identity = { id, name: null, email: null };
            records.write("session", identity);
            return ok(identity);
        },

        async signOut() {
            records.remove("session");
            return ok(undefined);
        },

        catalog: async (course) =>
            signedIn(() => {
                if (course !== content.course) return refused(404);
                return ok(content.exercises.map(({ id }) => ({ id, open: true, opens_at: null, closes_at: null })));
            }),

        statement: async (course, id) => gate(course, id, (_, exercise) => ok(exercise.statement)),

        history: async (course, id) => gate(course, id, (student) => ok(runs.of({ student, course, exercise: id }))),

        draft: async (course, id) => gate(course, id, (student) => ok(readDraft(records, draftKey(student, course, id)))),

        saveDraft: async (course, id, write) =>
            gate(course, id, (student) => {
                const saved = writeDraft(records, draftKey(student, course, id), write);
                // 422 is how the API refuses a draft that is too large, and the page says so
                return saved ? ok(saved) : refused(422);
            }),

        submit: async ({ course, exercise: id, language_pack, source }) =>
            gate(course, id, (student, exercise) => {
                // Pyodide runs Python only
                if (language_pack !== "python") return refused(422);
                return ok({ id: runs.start({ student, course, exercise: id }, exercise.cases, source) });
            }),

        progress: async (id) =>
            signedIn((student) => {
                const run = runs.find(student, id);
                return run ? ok(run) : refused(404);
            }),
    };

    // The stream never stops, so the page never falls back to polling: `onClosed` is not called
    return { api, followRuns: (onRun) => runs.follow(onRun) };
}
