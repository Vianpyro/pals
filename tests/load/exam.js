// N students on one exercise, at the rates assumed by docs/architecture.typ (target load).
// Usage: node tests/load/exam.js <api url> <students> <steady|burst> <seconds> <pack> <source file>
// Prints the HTTP side of the measure as JSON; the queue's side is read from the jobs table.
// steady: each student follows their runs, saves drafts and submits. burst: all submit at once.
// shortcut: sign-in is skipped (run.sh opens the sessions), so a sign-in storm is not measured; add it when LDAP's capacity is known.
import { readFileSync } from "node:fs";

const SUBMIT_EVERY = 180;
const DRAFT_EVERY = 2.5; // The document's 0.4 writes per second per student, all counted as drafts
const COURSE = "log200";
const EXERCISE = "sum";

const [url, studentsArg, mode, secondsArg, pack, sourceFile] = process.argv.slice(2);
const students = Number(studentsArg);
const stop = Date.now() + Number(secondsArg) * 1000;
const source = readFileSync(sourceFile, "utf8");
const leaving = new AbortController();
const latencies = [];
let errors = 0;

const exponential = (mean) => -Math.log(1 - Math.random()) * mean;
const sleep = (seconds) => new Promise((resolve) => setTimeout(resolve, seconds * 1000));
const headers = (student) => ({ "content-type": "application/json", cookie: `__Host-session=load-${student}` });

async function request(student, path, method, body) {
    const start = performance.now();
    try {
        const response = await fetch(`${url}${path}`, { method, headers: headers(student), body: JSON.stringify(body) });
        const text = await response.text();
        latencies.push(performance.now() - start);
        if (response.ok) return text;
        console.error(`${method} ${path}: ${response.status}`);
    } catch (error) {
        console.error(`${method} ${path}: ${error.cause?.code ?? error.message}`);
    }
    errors++;
    return null;
}

const submit = (student) => request(student, "/submissions", "POST", {
    course: COURSE,
    exercise: EXERCISE,
    language_pack: pack,
    source,
});

// Every interval is exponential, so students do not fall into step with each other
async function every(mean, action) {
    for (;;) {
        const wait = exponential(mean);
        if (Date.now() + wait * 1000 >= stop) return;
        await sleep(wait);
        await action();
    }
}

async function drafting(student) {
    let base = 0;
    await every(DRAFT_EVERY, async () => {
        const saved = await request(student, `/courses/${COURSE}/exercises/${EXERCISE}/draft`, "PUT", {
            base,
            source: `${source}\n# ${base}`,
        });
        if (saved) base = JSON.parse(saved).revision;
    });
}

// The API ends a stream after 5 minutes, and the page opens another
async function following(student) {
    while (!leaving.signal.aborted) {
        try {
            const response = await fetch(`${url}/events`, { headers: headers(student), signal: leaving.signal });
            if (!response.ok) throw new Error(`status ${response.status}`);
            for await (const _ of response.body); // eslint-disable-line no-unused-vars
        } catch (error) {
            if (leaving.signal.aborted) return;
            console.error(`GET /events: ${error.cause?.code ?? error.message}`);
            errors++;
            await sleep(1);
        }
    }
}

const everyone = Array.from({ length: students }, (_, index) => index + 1);
if (mode === "burst") {
    await Promise.all(everyone.map(submit));
} else {
    const follows = everyone.map(following);
    await Promise.all(everyone.flatMap((student) => [every(SUBMIT_EVERY, () => submit(student)), drafting(student)]));
    leaving.abort();
    await Promise.all(follows);
}

latencies.sort((a, b) => a - b);
console.log(JSON.stringify({
    requests: latencies.length + errors,
    errors,
    p95_ms: Math.round(latencies[Math.floor(latencies.length * 0.95)] ?? 0),
}));
process.exit(0); // Pooled keep-alive connections would hold the process open
