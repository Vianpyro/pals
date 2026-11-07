// The exercises the demo ships, from the files of examples/content (instance.ts reads them at build time).
// Hidden cases included: the examples are public. Real course content must never come through here.
import type { Case } from "./judging.ts";

export type Exercise = { id: string; statement: string; cases: Case[] };

const STATEMENT = /\/exercises\/([^/]+)\/statement\.md$/;
const INPUT = /\/exercises\/([^/]+)\/assessment\/([^/]+)\/[^/]+\.in$/;

// Visible cases first, each kind in name order, as apps/judge/src/cases.rs numbers them
const rank = ({ visible }: Case) => (visible ? 0 : 1);
const byName = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);

/** `files` maps paths to contents. Only Markdown statements: Typst and LaTeX need the publisher. */
export function readExercises(files: Record<string, string>): Exercise[] {
    const exercises = new Map<string, Exercise>();
    for (const [path, statement] of Object.entries(files)) {
        const id = STATEMENT.exec(path)?.[1];
        if (id) exercises.set(id, { id, statement, cases: [] });
    }
    const inputs: { path: string; exercise: Exercise; case: Case }[] = [];
    for (const [path, input] of Object.entries(files)) {
        const [, id = "", kind] = INPUT.exec(path) ?? [];
        const exercise = exercises.get(id);
        if (!exercise) continue;
        if (kind !== "visible" && kind !== "hidden") throw new Error(`Case outside visible/ and hidden/: ${path}`);
        const expected = files[path.replace(/\.in$/, ".out")];
        if (expected === undefined) throw new Error(`No expected output for ${path}`);
        inputs.push({ path, exercise, case: { input, expected, visible: kind === "visible" } });
    }
    inputs.sort((a, b) => rank(a.case) - rank(b.case) || byName(a.path, b.path));
    for (const { exercise, case: found } of inputs) exercise.cases.push(found);
    return [...exercises.values()].sort((a, b) => a.id.localeCompare(b.id));
}
