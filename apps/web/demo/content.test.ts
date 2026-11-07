import assert from "node:assert/strict";
import { test } from "node:test";
import { readExercises } from "./content.ts";

const root = "../../../examples/content/exercises";

test("reads each exercise with its statement and its visible and hidden cases", () => {
    const exercises = readExercises({
        [`${root}/sum/statement.md`]: "# Somme",
        [`${root}/sum/assessment/visible/1.in`]: "1 2\n",
        [`${root}/sum/assessment/visible/1.out`]: "3\n",
        [`${root}/sum/assessment/hidden/2.in`]: "2 2\n",
        [`${root}/sum/assessment/hidden/2.out`]: "4\n",
    });

    assert.deepEqual(exercises, [
        {
            id: "sum",
            statement: "# Somme",
            cases: [
                { input: "1 2\n", expected: "3\n", visible: true },
                { input: "2 2\n", expected: "4\n", visible: false },
            ],
        },
    ]);
});

test("puts the visible cases first, each kind in name order, as the judge numbers them", () => {
    const [{ cases }] = readExercises({
        [`${root}/sum/statement.md`]: "",
        [`${root}/sum/assessment/hidden/a.in`]: "hidden a",
        [`${root}/sum/assessment/hidden/a.out`]: "",
        [`${root}/sum/assessment/visible/b.in`]: "visible b",
        [`${root}/sum/assessment/visible/b.out`]: "",
        [`${root}/sum/assessment/visible/a.in`]: "visible a",
        [`${root}/sum/assessment/visible/a.out`]: "",
    });
    assert.deepEqual(
        cases.map(({ input }) => input),
        ["visible a", "visible b", "hidden a"],
    );
});

test("lists the exercises in order of their identifiers", () => {
    const exercises = readExercises({
        [`${root}/zeta/statement.md`]: "",
        [`${root}/alpha/statement.md`]: "",
    });
    assert.deepEqual(
        exercises.map((exercise) => exercise.id),
        ["alpha", "zeta"],
    );
});

test("refuses a case outside visible/ and hidden/, as the judge does", () => {
    assert.throws(
        () =>
            readExercises({
                [`${root}/sum/statement.md`]: "",
                [`${root}/sum/assessment/other/1.in`]: "1 2\n",
                [`${root}/sum/assessment/other/1.out`]: "3\n",
            }),
        /sum.*other.*1\.in/,
    );
});

test("refuses a case whose expected output is missing", () => {
    assert.throws(
        () =>
            readExercises({
                [`${root}/sum/statement.md`]: "",
                [`${root}/sum/assessment/visible/1.in`]: "1 2\n",
            }),
        /sum.*1\.in/,
    );
});
