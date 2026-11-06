import assert from "node:assert/strict";
import { test } from "node:test";
import { capacity, checkBudgets, report, size, waitP95, worstBurst } from "./sizing.js";

const near = (actual, expected, margin = 0.05) =>
    assert.ok(Math.abs(actual - expected) <= margin, `${actual} is not about ${expected}`);

// The hypotheses of docs/architecture.typ: one submission every 3 minutes, 2 s per job
const T = 180;
const S = 2;

test("the P95 wait follows Erlang C", () => {
    near(waitP95({ students: 250, service: S, period: T, judges: 4 }), 3.5);
    near(waitP95({ students: 1250, service: S, period: T, judges: 20 }), 0.2);
});

test("most jobs never wait when few are likely to", () => {
    assert.equal(waitP95({ students: 250, service: S, period: T, judges: 7 }), 0);
});

test("judges that cannot keep up have no steady wait", () => {
    assert.equal(waitP95({ students: 250, service: S, period: T, judges: 2 }), Infinity);
    // Exactly at capacity the queue never settles either
    assert.equal(waitP95({ students: 90, service: S, period: T, judges: 1 }), Infinity);
});

test("the queue holds one waiting job per student at most, so a burst is bounded", () => {
    assert.equal(worstBurst({ students: 250, service: S, judges: 4 }), 125);
});

const memory = {
    api_idle: 10,
    api: 30,
    judge_each: 8,
    overhead_each: 40,
    sandboxes: 100,
    postgres_idle: 60,
    postgres: 160,
};

// A burst's queue never empties until the end, so its rate is what those judges can sustain
const burst = (pack, judges, rate) => ({
    students: 100,
    judges,
    mode: "burst",
    pack,
    host_cores: 16,
    jobs: { done: rate * 100, replaced: 0, failed: 0, wait_p95_s: 40, wait_max_s: 95, service_mean_s: judges / rate },
    http: { requests: 100, errors: 0, p95_ms: 500 },
    memory_mib: memory,
    cpu_s: { api: 1, judges: 50, sandboxes: 100, postgres: 2 },
    disk_mib: { database_growth: 1 },
    duration_s: 100,
});
const steady = {
    students: 100,
    judges: 4,
    mode: "steady",
    pack: "python",
    host_cores: 16,
    jobs: { done: 200, replaced: 0, failed: 0, wait_p95_s: 1, wait_max_s: 2, service_mean_s: S },
    http: { requests: 5000, errors: 0, p95_ms: 20 },
    memory_mib: memory,
    cpu_s: { api: 30, judges: 20, sandboxes: 200, postgres: 60 },
    disk_mib: { database_growth: 2 },
    duration_s: 300,
};
const python = [burst("python", 1, 0.43), burst("python", 2, 0.88), burst("python", 4, 1.42), burst("python", 8, 1.36)];
const java = [burst("java", 1, 0.11), burst("java", 2, 0.21), burst("java", 4, 0.41)];
const limits = { sandbox_mib: 256, utilization: 0.7, period: T };
const profiles = [
    { name: "Minimale", students: 100, spare: 0 },
    { name: "Recommandée", students: 1000, spare: 1 },
];

test("a machine's ceiling is the fewest judges that reach its best rate", () => {
    const ceiling = capacity(python, "python");
    assert.equal(ceiling.judges, 4);
    assert.equal(ceiling.rate, 1.42);
    assert.equal(ceiling.reached, true);
    near(ceiling.service, 4 / 1.42);
});

test("a rate still rising at the most judges measured is no ceiling", () => {
    const ceiling = capacity(java, "java");
    assert.equal(ceiling.judges, 4);
    assert.equal(ceiling.reached, false);
});

test("the pack with the lowest rate sets the configuration", () => {
    const [minimum] = size([steady, ...python, ...java], profiles, limits);
    assert.equal(minimum.pack, "java");
});

test("demand under the target utilization sets the machines, then their judges", () => {
    // 100 students every 180 s, at 70 %: 0.79 jobs per second
    const [minimum, recommended] = size([steady, ...python], profiles, limits);
    assert.equal(minimum.judge.vms, 1);
    assert.equal(minimum.judge.judges, 4);
    // 7.94 jobs per second at 1.42 per machine
    assert.equal(recommended.judge.vms, 6);
    assert.equal(recommended.judge.judges, 24);
});

test("a machine reserves the sandbox limit, since a student may reach it", () => {
    const [minimum] = size([steady, ...python], profiles, limits);
    assert.equal(minimum.judge.memory_mib_per_vm, 4 * (256 + 40 + 8));
    // One core per sandbox, and what the judges used: 50 s of 100 s
    assert.equal(minimum.judge.cores_per_vm, 4 + 1);
});

test("the wait comes from the service time at the ceiling", () => {
    const [minimum] = size([steady, ...python], profiles, limits);
    const service = 4 / 1.42;
    near(minimum.judge.wait_p95_s, waitP95({ students: 100, service, period: T, judges: 4 }));
    near(minimum.judge.worst_burst_s, 100 / 1.42);
});

test("only a single machine that held the load is measured", () => {
    const [minimum, recommended] = size([steady, ...python], profiles, limits);
    assert.equal(minimum.basis, "mesuré");
    assert.equal(recommended.basis, "extrapolé");
    const errors = { ...steady, http: { ...steady.http, errors: 3 } };
    assert.equal(size([errors, ...python], profiles, limits)[0].basis, "extrapolé");
    const otherPack = { ...steady, pack: "java" };
    assert.equal(size([otherPack, ...python], profiles, limits)[0].basis, "extrapolé");
});

test("web and database scale from idle with the number of students", () => {
    const [minimum, recommended] = size([steady, ...python], profiles, limits);
    assert.equal(minimum.web.memory_mib, 30);
    assert.equal(recommended.web.memory_mib, 10 + 10 * (30 - 10));
    assert.equal(recommended.db.memory_mib, 60 + 10 * (160 - 60));
    // 30 s of API over 300 s for 100 students, at 70 %
    near(minimum.web.cores_exact, 30 / 300 / 0.7);
    near(recommended.web.cores_exact, (10 * 30) / 300 / 0.7);
});

test("the database grows with the jobs of a one-hour exam", () => {
    const [minimum] = size([steady, ...python], profiles, limits);
    // 2 MiB per 200 jobs, and 100 students submit 20 times an hour
    near(minimum.db.disk_mib_per_hour, 20);
});

test("a spare judge machine adds to the recommended configuration only", () => {
    const [minimum, recommended] = size([steady, ...python], profiles, limits);
    assert.equal(minimum.judge.spare_vms, 0);
    assert.equal(recommended.judge.spare_vms, 1);
});

test("a pack whose jobs all failed has no rate to size from, and says so", () => {
    const failed = burst("cobol", 1, 0.5);
    failed.jobs = { ...failed.jobs, done: 0, failed: 50 };
    assert.throws(() => size([steady, ...python, failed], profiles, limits), /cobol/);
});

test("sizing needs a steady measure and a burst", () => {
    assert.throws(() => size(python, profiles, limits), /steady/);
    assert.throws(() => size([steady], profiles, limits), /burst/);
});

test("the report compares the model with what was measured", () => {
    const text = report([steady, ...python, ...java], profiles, limits);
    assert.match(text, /Minimale.*extrapolé/);
    assert.match(text, /16 cœurs/);
    assert.match(text, /steady.*python/);
    // 100 students × 2.82 s ÷ 4 judges, against the 95 s measured
    assert.match(text, /burst \| python \| 100 \| 4 .*95\.0 s \| 70\.4 s/);
    assert.match(text, /java.*plafond non atteint/);
});

test("a measure over its budget fails, and a missing one is named", () => {
    const budget = { api: 40, judge_each: 16, overhead_each: 50, postgres: 200 };
    assert.deepEqual(checkBudgets([steady], budget), []);
    assert.deepEqual(checkBudgets([steady], { ...budget, api: 20 }), ["api: 30 Mio > 20 Mio (steady python)"]);
    assert.deepEqual(checkBudgets([steady], { nope: 1 }), ["nope: absent de steady python"]);
});
