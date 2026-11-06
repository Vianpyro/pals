import { defineConfig, devices } from "@playwright/test";

// The demo as GitHub Pages serves it: plain files from a subfolder, with no API behind them.
// Run with `npm run e2e:demo`, which builds it first. Pyodide comes from its CDN, so this needs the network.
const PORT = 4173;
const baseURL = `http://localhost:${PORT}/dist-demo/`;

export default defineConfig({
    testDir: "tests/demo",
    forbidOnly: Boolean(process.env.CI),
    // One journey stalled once in 120 runs, cause unknown: a slow CDN should not fail the whole CI job. Reported as flaky.
    retries: process.env.CI ? 1 : 0,
    reporter: "list",
    use: { baseURL, locale: "fr-CA", trace: "retain-on-failure" },
    webServer: {
        command: `python3 -m http.server ${PORT} --directory apps/web`,
        url: baseURL,
        reuseExistingServer: !process.env.CI,
    },
    projects: [
        { name: "chromium", use: devices["Desktop Chrome"] },
        { name: "webkit", use: devices["Desktop Safari"] },
    ],
});
