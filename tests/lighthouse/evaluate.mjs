// From Lighthouse's flow results to a verdict and its report. No I/O here: evaluate.test.mjs
// checks all of it without a browser.

const CATEGORIES = new Set(["performance", "accessibility", "best-practices", "seo"]);

/**
 * What one flow measured, step by step.
 * @param {{steps: {name: string, lhr: any}[]}} flowResult
 * @returns {{name: string, mode: string, values: Record<string, number>, failing: Record<string, string[]>}[]}
 */
export function measure(flowResult) {
    return flowResult.steps.map(({ name, lhr }) => {
        const values = {};
        const failing = {};
        for (const [id, category] of Object.entries(lhr.categories)) {
            if (typeof category.score === "number") values[id] = category.score;
            // The weighted audits that kept the score under 1
            failing[id] = category.auditRefs
                .filter((ref) => ref.weight > 0)
                .map((ref) => lhr.audits[ref.id])
                .filter((audit) => typeof audit?.score === "number" && audit.score < 1)
                .map((audit) => audit.title);
        }
        for (const [id, audit] of Object.entries(lhr.audits)) {
            if (typeof audit.numericValue === "number") values[id] = audit.numericValue;
        }
        // The elements that moved, worst first
        failing["cumulative-layout-shift"] = (lhr.audits["layout-shifts"]?.details?.items ?? [])
            .filter((item) => item.node?.selector)
            .sort((a, b) => b.score - a.score)
            .map((item) => `${item.node.selector} moved (${item.score.toFixed(3)})`);
        return { name, mode: lhr.gatherMode, values, failing };
    });
}

/** @param {number[]} numbers */
function middle(numbers) {
    const sorted = [...numbers].sort((a, b) => a - b);
    const half = Math.floor(sorted.length / 2);
    return sorted.length % 2 ? sorted[half] : (sorted[half - 1] + sorted[half]) / 2;
}

/**
 * The median of every value over the runs, step by step; the audits that failed in any run.
 * Every run goes through the same steps, so they line up by position.
 * @param {ReturnType<typeof measure>[]} runs
 */
export function median(runs) {
    return runs[0].map((step, index) => {
        const same = runs.map((run) => run[index]);
        const values = {};
        for (const id of Object.keys(step.values)) {
            const measured = same.map((run) => run.values[id]).filter((value) => typeof value === "number");
            if (measured.length === same.length) values[id] = middle(measured);
        }
        const failing = {};
        for (const id of Object.keys(step.failing)) {
            failing[id] = [...new Set(same.flatMap((run) => run.failing[id] ?? []))];
        }
        return { name: step.name, mode: step.mode, values, failing };
    });
}

/**
 * One row per budget and step it applies to. A value missing from an "error" budget fails: a gate
 * that could not measure must not pass.
 * @param {ReturnType<typeof median>} steps
 * @param {{metric: string, modes: string[], level: "error" | "warn", max?: number, min?: number}[]} budgets
 */
export function judge(steps, budgets) {
    return budgets.flatMap((budget) =>
        steps
            .filter((step) => budget.modes.includes(step.mode))
            .map((step) => {
                const value = step.values[budget.metric];
                const measured = typeof value === "number";
                const ok =
                    measured &&
                    (budget.max === undefined || value <= budget.max) &&
                    (budget.min === undefined || value >= budget.min);
                return {
                    step: step.name,
                    metric: budget.metric,
                    level: budget.level,
                    value: measured ? value : undefined,
                    limit: budget.max !== undefined ? `≤ ${show(budget.metric, budget.max)}` : `≥ ${show(budget.metric, budget.min)}`,
                    ok,
                    failing: step.failing[budget.metric] ?? [],
                };
            }),
    );
}

/** @param {string} metric @param {number | undefined} value */
export function show(metric, value) {
    if (value === undefined) return "not measured";
    if (CATEGORIES.has(metric)) return String(Math.round(value * 100));
    if (metric === "cumulative-layout-shift") return value.toFixed(3);
    return `${Math.round(value)} ms`;
}

/** @param {ReturnType<typeof judge>[number]} row */
function mark(row) {
    if (row.ok) return "✅";
    return row.level === "error" ? "❌" : "⚠️";
}

/**
 * The Markdown a reviewer reads in the run's summary: a verdict first, then every row.
 * @param {string} profile @param {{width: number, height: number}} screen @param {number} runs
 * @param {ReturnType<typeof judge>} rows
 */
export function report(profile, screen, runs, rows) {
    const failed = rows.filter((row) => !row.ok && row.level === "error").length;
    const warned = rows.filter((row) => !row.ok && row.level === "warn").length;
    const verdict = failed
        ? `❌ ${failed} budget${failed > 1 ? "s" : ""} exceeded`
        : warned
          ? `✅ budgets held, ⚠️ ${warned} warning${warned > 1 ? "s" : ""}`
          : "✅ every budget held";
    const lines = [
        `### Lighthouse, ${profile} (${screen.width}×${screen.height}): ${verdict}`,
        "",
        `Median of ${runs} run${runs > 1 ? "s" : ""}. The full reports are in the \`lighthouse-${profile}\` artifact.`,
        "",
        "| | Step | Metric | Measured | Budget |",
        "|---|---|---|---|---|",
        ...rows.map(
            (row) =>
                `| ${mark(row)} | ${row.step} | ${row.metric} | ${show(row.metric, row.value)} | ${row.limit}${row.level === "warn" ? " (warn)" : ""} |`,
        ),
    ];
    const explained = rows.filter((row) => !row.ok && row.failing.length);
    if (explained.length) {
        lines.push("", "What to look at:", "");
        for (const row of explained) lines.push(`- ${row.step}, ${row.metric}: ${row.failing.join("; ")}`);
    }
    return `${lines.join("\n")}\n`;
}

/**
 * GitHub's workflow commands, so a broken budget shows on the run and on the pull request.
 * @param {string} profile @param {ReturnType<typeof judge>} rows
 */
export function annotations(profile, rows) {
    // %, \r and \n end or break a command, and : and , a property: GitHub's own escaping
    const escape = (text) => text.replace(/%/g, "%25").replace(/\r/g, "%0D").replace(/\n/g, "%0A");
    const property = (text) => escape(text).replace(/:/g, "%3A").replace(/,/g, "%2C");
    return rows
        .filter((row) => !row.ok)
        .map((row) => {
            const kind = row.level === "error" ? "error" : "warning";
            const title = property(`Lighthouse ${profile}: ${row.metric}`);
            const why = row.failing.length ? ` (${row.failing.join("; ")})` : "";
            return `::${kind} title=${title}::${escape(`${row.step}: ${show(row.metric, row.value)}, budget ${row.limit}${why}`)}`;
        });
}
