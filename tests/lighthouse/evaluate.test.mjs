import assert from "node:assert/strict";
import { test } from "node:test";
import { annotations, judge, measure, median, report, show } from "./evaluate.mjs";

// The shape of a Lighthouse flow result, reduced to what evaluate.mjs reads
function step(name, gatherMode, { cls, accessibility = 1, contrast = 1 }) {
    return {
        name,
        lhr: {
            gatherMode,
            categories: {
                accessibility: {
                    score: accessibility,
                    auditRefs: [
                        { id: "color-contrast", weight: 7 },
                        { id: "an-unweighted-audit", weight: 0 },
                    ],
                },
            },
            audits: {
                "color-contrast": { title: "Contrast", score: contrast },
                "an-unweighted-audit": { title: "Unweighted", score: 0 },
                ...(cls === undefined ? {} : { "cumulative-layout-shift": { numericValue: cls } }),
            },
        },
    };
}

const flow = (cls, extra = {}) => ({
    steps: [step("Sign-in page", "navigation", { cls, ...extra }), step("Catalog", "snapshot", {})],
});

const BUDGETS = [
    { metric: "cumulative-layout-shift", modes: ["navigation"], level: "error", max: 0.05 },
    { metric: "accessibility", modes: ["navigation", "snapshot"], level: "error", min: 1 },
];

test("a step keeps its name, mode, scores and numeric values", () => {
    const [signIn, catalog] = measure(flow(0.01));
    assert.equal(signIn.name, "Sign-in page");
    assert.equal(signIn.mode, "navigation");
    assert.equal(signIn.values["cumulative-layout-shift"], 0.01);
    assert.equal(signIn.values.accessibility, 1);
    assert.equal(catalog.mode, "snapshot");
});

test("a failing audit is named only when it weighs on its category", () => {
    const [signIn] = measure(flow(0, { accessibility: 0.93, contrast: 0 }));
    assert.deepEqual(signIn.failing.accessibility, ["Contrast"]);
});

test("a layout shift names the elements that moved, worst first", () => {
    const flowResult = flow(0.06);
    flowResult.steps[0].lhr.audits["layout-shifts"] = {
        details: {
            items: [
                { score: 0.01, node: { selector: "body > aside" } },
                { score: 0.05, node: { selector: "body > main" } },
                { score: 0.02 },
            ],
        },
    };
    const [signIn] = measure(flowResult);
    assert.deepEqual(signIn.failing["cumulative-layout-shift"], [
        "body > main moved (0.050)",
        "body > aside moved (0.010)",
    ]);
});

test("the median is taken step by step, odd and even", () => {
    const runs = [0.3, 0.1, 0.2].map((cls) => measure(flow(cls)));
    assert.equal(median(runs)[0].values["cumulative-layout-shift"], 0.2);
    const even = [0.4, 0.1, 0.3, 0.2].map((cls) => measure(flow(cls)));
    assert.equal(median(even)[0].values["cumulative-layout-shift"], 0.25);
});

test("a value missing from one run is not measured, rather than a median of fewer runs", () => {
    const runs = [measure(flow(0.01)), measure(flow(undefined))];
    assert.equal(median(runs)[0].values["cumulative-layout-shift"], undefined);
});

test("an audit that failed in any run is kept", () => {
    const runs = [measure(flow(0)), measure(flow(0, { accessibility: 0.9, contrast: 0 }))];
    assert.deepEqual(median(runs)[0].failing.accessibility, ["Contrast"]);
});

test("a budget applies to its modes only, and holds at its limit", () => {
    const rows = judge(median([measure(flow(0.05))]), BUDGETS);
    assert.deepEqual(
        rows.map((row) => [row.step, row.metric, row.ok]),
        [
            ["Sign-in page", "cumulative-layout-shift", true],
            ["Sign-in page", "accessibility", true],
            ["Catalog", "accessibility", true],
        ],
    );
});

test("over a maximum or under a minimum fails, and says why", () => {
    const rows = judge(median([measure(flow(0.06, { accessibility: 0.9, contrast: 0 }))]), BUDGETS);
    const failed = rows.filter((row) => !row.ok);
    assert.deepEqual(
        failed.map((row) => row.metric),
        ["cumulative-layout-shift", "accessibility"],
    );
    assert.deepEqual(failed[1].failing, ["Contrast"]);
});

test("a gate that could not measure fails", () => {
    const [row] = judge(median([measure(flow(undefined))]), BUDGETS);
    assert.equal(row.value, undefined);
    assert.equal(row.ok, false);
});

test("values read as a person would", () => {
    assert.equal(show("accessibility", 0.934), "93");
    assert.equal(show("cumulative-layout-shift", 0.0123), "0.012");
    assert.equal(show("largest-contentful-paint", 1234.6), "1235 ms");
    assert.equal(show("total-blocking-time", undefined), "not measured");
});

test("the report gives the verdict first, then each row and what to look at", () => {
    const rows = judge(median([measure(flow(0.06, { accessibility: 0.9, contrast: 0 }))]), BUDGETS);
    const text = report("laptop", { width: 1366, height: 768 }, 3, rows);
    assert.match(text.split("\n")[0], /^### Lighthouse, laptop \(1366×768\): ❌ 2 budgets exceeded$/);
    assert.match(text, /\| ❌ \| Sign-in page \| cumulative-layout-shift \| 0\.060 \| ≤ 0\.050 \|/);
    assert.match(text, /\| ✅ \| Catalog \| accessibility \| 100 \| ≥ 100 \|/);
    assert.match(text, /- Sign-in page, accessibility: Contrast/);
});

test("a warning does not count as an exceeded budget", () => {
    const warn = [{ metric: "cumulative-layout-shift", modes: ["navigation"], level: "warn", max: 0.05 }];
    const text = report("lab", { width: 1920, height: 1080 }, 1, judge(median([measure(flow(0.06))]), warn));
    assert.match(text, /✅ budgets held, ⚠️ 1 warning$/m);
    assert.match(text, /\(warn\) \|$/m);
});

test("annotations are GitHub workflow commands, escaped", () => {
    const rows = judge(median([measure(flow(0.06))]), BUDGETS);
    const [line] = annotations("laptop", rows);
    assert.equal(
        line,
        "::error title=Lighthouse laptop%3A cumulative-layout-shift::Sign-in page: 0.060, budget ≤ 0.050",
    );
    assert.deepEqual(annotations("laptop", judge(median([measure(flow(0))]), BUDGETS)), []);
});
