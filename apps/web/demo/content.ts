// The exercises the demo ships, from the files of examples/content (instance.ts reads them at build time).
// Hidden cases included: the examples are public. Real course content must never come through here.
import type { Case } from "./judging.ts";

export type Exercise = { id: string; statement: string; cases: Case[] };

const STATEMENT = /\/exercises\/([^/]+)\/statement\.md$/;
const INPUT = /\/exercises\/([^/]+)\/assessment\/[^/]+\/[^/]+\.in$/;

/** `files` maps paths to contents. Only Markdown statements: Typst and LaTeX need the publisher. */
export function readExercises(files: Record<string, string>): Exercise[] {
    const exercises = new Map<string, Exercise>();
    for (const [path, statement] of Object.entries(files)) {
        const id = STATEMENT.exec(path)?.[1];
        if (id) exercises.set(id, { id, statement, cases: [] });
    }
    for (const [path, input] of Object.entries(files)) {
        const exercise = exercises.get(INPUT.exec(path)?.[1] ?? "");
        if (!exercise) continue;
        const expected = files[path.replace(/\.in$/, ".out")];
        if (expected === undefined) throw new Error(`No expected output for ${path}`);
        exercise.cases.push({ input, expected });
    }
    return [...exercises.values()].sort((a, b) => a.id.localeCompare(b.id));
}
