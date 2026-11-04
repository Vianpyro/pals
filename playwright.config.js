import { defineConfig, devices } from "@playwright/test";

// The student's journeys against the dev stack (spec D12): nginx with its real CSP, the API, the judge.
// Needs the demo running (demo.sh) and is run from the dev container, where nginx is localhost.
// HTTPS with the dev container's self-signed certificate: WebKit keeps the Secure session cookie only over HTTPS.
export default defineConfig({
    testDir: "tests/e2e",
    forbidOnly: Boolean(process.env.CI),
    reporter: "list",
    use: {
        baseURL: process.env.BASE_URL ?? "https://localhost",
        ignoreHTTPSErrors: true,
        locale: "fr-CA",
        trace: "retain-on-failure",
    },
    // The two engines of Safe Exam Browser: Chromium on Windows, WebKit on macOS.
    // One account each: a run replaces the same student's run still waiting (ADR-0030).
    projects: [
        { name: "chromium", use: { ...devices["Desktop Chrome"], student: "etudiant1" } },
        { name: "webkit", use: { ...devices["Desktop Safari"], student: "etudiant2" } },
    ],
});
