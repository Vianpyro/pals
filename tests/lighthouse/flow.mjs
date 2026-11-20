// Measures the demo page as a student goes through it: the cold load of the sign-in page, signing
// in, the catalog, opening an exercise, the exercise. A Lighthouse user flow rather than page loads:
// the page is one document, and what a student sees jump happens after an action, not on load.
// Run by run.sh, which builds the demo and names the browser.
//
// Environment: LH_PROFILE (budgets.mjs, laptop by default), LH_RUNS (3), LH_OUT (where the reports
// go), LH_SITE (the built demo), CHROME_PATH (the browser, required).
import { appendFile, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { desktopConfig, startFlow } from "lighthouse";
import puppeteer from "puppeteer-core";
import { budgets, PROFILES, THROTTLING } from "./budgets.mjs";
import { annotations, judge, measure, median, report } from "./evaluate.mjs";
import { serve } from "./serve.mjs";

const env = process.env;
const name = env.LH_PROFILE || "laptop";
const screen = PROFILES[name];
const runs = Number(env.LH_RUNS || 3);
const out = env.LH_OUT || join(tmpdir(), "lighthouse");
const site = env.LH_SITE || "apps/web/dist-demo";

if (!screen) fail(`unknown LH_PROFILE "${name}"; expected one of ${Object.keys(PROFILES).join(", ")}`);
if (!Number.isInteger(runs) || runs < 1) fail(`LH_RUNS must be a positive integer, got "${env.LH_RUNS}"`);
if (!env.CHROME_PATH) fail("CHROME_PATH must name a Chrome or Chromium");

// The demo's own account, printed on the page itself (apps/web/vite.demo.config.js)
const ACCOUNT = { id: "etudiant1", password: "log791-dev" };
// By accessible name and role, as the e2e journeys find them
const aria = (label, role) => `::-p-aria([name="${label}"][role="${role}"])`;

const settings = {
    formFactor: "desktop",
    screenEmulation: { ...screen, mobile: false, disabled: false },
    throttlingMethod: "devtools",
    throttling: THROTTLING,
};
// Lighthouse refuses accessibility in a timespan. No SEO: nothing here is meant to be indexed.
const whole = { onlyCategories: ["performance", "accessibility", "best-practices"] };
const action = { onlyCategories: ["performance", "best-practices"] };

await mkdir(out, { recursive: true });
const server = await serve(site, "/demo/");
try {
    const measured = [];
    for (let run = 1; run <= runs; run++) {
        const started = Date.now();
        const { flowResult, html } = await journey(server.url);
        await writeFile(join(out, `${name}-${run}.html`), html);
        await writeFile(join(out, `${name}-${run}.json`), JSON.stringify(flowResult));
        measured.push(measure(flowResult));
        console.log(`run ${run}/${runs}: ${Math.round((Date.now() - started) / 1000)} s`);
    }

    const rows = judge(median(measured), budgets(name));
    const text = report(name, screen, runs, rows);
    console.log(`\n${text}`);
    if (env.GITHUB_ACTIONS) for (const line of annotations(name, rows)) console.log(line);
    if (env.GITHUB_STEP_SUMMARY) await appendFile(env.GITHUB_STEP_SUMMARY, text);
    console.log(`Reports: ${out}`);
    process.exitCode = rows.some((row) => !row.ok && row.level === "error") ? 1 : 0;
} finally {
    await server.close();
}

/** One student's way through the demo, in a browser of its own: nothing cached from a run before. */
async function journey(url) {
    const browser = await puppeteer.launch({
        executablePath: env.CHROME_PATH,
        // French: the page's main language, and a chunk of its own that English does not load.
        // --accept-lang, not --lang: headless Chrome ignores --lang and says en-US.
        // No sandbox, as Playwright launches the same Chromium: the runner's AppArmor and the dev
        // container refuse it the user namespaces it needs. The only page it opens is our own.
        args: ["--accept-lang=fr-CA,fr", "--no-sandbox"],
        defaultViewport: null,
    });
    try {
        const page = await browser.newPage();
        const flow = await startFlow(page, { name: `LOG791 demo, ${name}`, config: desktopConfig, flags: settings });

        await flow.navigate(url, { ...whole, name: "Sign-in page" });

        await flow.startTimespan({ ...action, name: "Sign in" });
        await page.locator(aria("Identifiant", "textbox")).fill(ACCOUNT.id);
        await page.locator(aria("Mot de passe", "textbox")).fill(ACCOUNT.password);
        await page.locator(aria("Se connecter", "button")).click();
        await page.locator(aria("sum", "button")).wait();
        await page.waitForNetworkIdle({ idleTime: 500 });
        await flow.endTimespan();
        await flow.snapshot({ ...whole, name: "Catalog" });

        await flow.startTimespan({ ...action, name: "Open an exercise" });
        await page.locator(aria("sum", "button")).click();
        // Editable once the exercise and the student's draft have arrived
        const editor = await page.locator(aria("Code Python", "textbox")).waitHandle();
        await page.waitForFunction((element) => !element.readOnly, {}, editor);
        await page.waitForNetworkIdle({ idleTime: 500 });
        await flow.endTimespan();
        await flow.snapshot({ ...whole, name: "Exercise" });

        return { flowResult: await flow.createFlowResult(), html: await flow.generateReport() };
    } finally {
        await browser.close();
    }
}

/** @param {string} message @returns {never} */
function fail(message) {
    console.error(`flow.mjs: ${message}`);
    process.exit(2);
}
