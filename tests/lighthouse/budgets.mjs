// What tests/lighthouse/flow.mjs measures and what it holds the page to, for every screen profile.

/**
 * Desktop screens only: phones are banned in the exam room, and Safe Exam Browser runs on the
 * students' laptops and on the lab computers (architecture, Safe Exam Browser).
 */
export const PROFILES = {
    // The smallest common laptop screen
    laptop: { width: 1366, height: 768, deviceScaleFactor: 1 },
    // A lab computer's monitor
    lab: { width: 1920, height: 1080, deviceScaleFactor: 1 },
};

/**
 * The same for every profile, or their numbers could not be compared. Devtools throttling, not
 * the simulated kind: a user flow can only measure what really happened.
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
 * CLS measured 0.000 at every step when these were set: 0.01 catches the first real shift (0.1 is
 * "good") and leaves room for sub-pixel rounding. Measure again (run.sh PROFILE) when the layout
 * changes on purpose.
 */
const CLS = { laptop: 0.01, lab: 0.01 };

/** @param {keyof typeof PROFILES} profile */
export function budgets(profile) {
    return [
        // Independent of the runner's speed, so it can fail the job
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
