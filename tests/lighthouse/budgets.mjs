// What tests/lighthouse/flow.mjs measures and what it holds the page to, for every screen profile.
// One file for all of them: near-identical copies would drift apart the first time a number moves.

/**
 * Desktop screens only: phones are banned in the exam room, and Safe Exam Browser runs on the
 * students' laptops and on the lab computers (architecture, Safe Exam Browser).
 */
export const PROFILES = {
    // The smallest screen still sold on student laptops
    laptop: { width: 1366, height: 768, deviceScaleFactor: 1 },
    // A lab computer's monitor
    lab: { width: 1920, height: 1080, deviceScaleFactor: 1 },
};

/**
 * The same for every profile, or their numbers could not be compared. Devtools throttling, not
 * the simulated kind: a user flow can only measure what really happened, and over loopback an
 * unthrottled page scores a flattering 100 that says nothing.
 */
// shortcut: a full exam room on one access point, and an entry-level laptop, guessed; to measure on an ÉTS lab computer
export const THROTTLING = {
    requestLatencyMs: 150,
    downloadThroughputKbps: 5000,
    uploadThroughputKbps: 1000,
    cpuSlowdownMultiplier: 4,
};

/**
 * A budget applies to the steps of the given modes: "navigation" is the cold load, "timespan" a
 * student's action, "snapshot" the screen it leads to. "error" fails the job; "warn" is reported,
 * since a shared runner moves the timings enough to fail at random.
 *
 * Nothing moves today: CLS measured 0.000 at every step, on both profiles (median of 3 runs,
 * Chromium 156, 2026-11-20). 0.01 catches the first real shift, a tenth of what Google calls good,
 * and leaves room for sub-pixel rounding. Measure again when the layout changes on purpose:
 * bash tests/lighthouse/run.sh PROFILE.
 */
const CLS = { laptop: 0.01, lab: 0.01 };

/** @param {keyof typeof PROFILES} profile */
export function budgets(profile) {
    return [
        // What a student sees jump under the cursor; the same whatever the runner's speed
        { metric: "cumulative-layout-shift", modes: ["navigation", "timespan"], level: "error", max: CLS[profile] },
        // axe in the e2e journeys checks WCAG 2.1 AA; Lighthouse adds its best-practice rules
        { metric: "accessibility", modes: ["navigation", "snapshot"], level: "error", min: 1 },
        { metric: "best-practices", modes: ["navigation", "snapshot"], level: "error", min: 1 },
        { metric: "performance", modes: ["navigation"], level: "warn", min: 0.9 },
        { metric: "largest-contentful-paint", modes: ["navigation"], level: "warn", max: 2500 },
        { metric: "total-blocking-time", modes: ["navigation", "timespan"], level: "warn", max: 200 },
        { metric: "interaction-to-next-paint", modes: ["timespan"], level: "warn", max: 200 },
    ];
}
