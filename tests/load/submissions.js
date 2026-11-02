// Exam submissions at an open arrival rate, each polled until its verdict (architecture: Target load)
// k6 run -e API=http://web:8080 -e RUN=load-250 -e STUDENTS=250 tests/load/submissions.js

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Trend } from 'k6/metrics';

const API = __ENV.API || 'http://localhost:8080';
const RUN = __ENV.RUN || 'load'; // Sent as the course: report.sql selects the run's jobs by it
const EXERCISE = __ENV.EXERCISE || 'sum';
const STUDENTS = Number(__ENV.STUDENTS || 250);
const INTERVAL = Number(__ENV.INTERVAL || 180); // Seconds between two submissions of a student
const DURATION = __ENV.DURATION || '30m';
const BURST = Number(__ENV.BURST || 2); // End-of-exam rate, as a multiple of the base rate
const BURST_DURATION = __ENV.BURST_DURATION || '10m';
const POLL = Number(__ENV.POLL || 2); // Seconds between two progress requests, as the UI will poll
const WAIT = 600; // Seconds before a student gives up on a verdict

// shortcut: assumed verdict mix, replace with the TCH009 CTester mix before the real run
const MIX = { passed: 40, wrong: 35, crashed: 15, 'timed-out': 7, memory: 2, flood: 1 };
const CORPUS = Object.entries(MIX).map(([name, weight]) => ({
  name,
  weight,
  source: open(`./corpus/${name}.py`),
}));
const TOTAL = CORPUS.reduce((total, entry) => total + entry.weight, 0);

const RATE = Math.ceil((STUDENTS * 60) / INTERVAL); // Submissions per minute

export const options = {
  scenarios: {
    exam: {
      executor: 'ramping-arrival-rate',
      timeUnit: '1m',
      startRate: RATE,
      // A student waits for their verdict before submitting again: k6 drops the excess iterations
      preAllocatedVUs: STUDENTS,
      maxVUs: STUDENTS,
      stages: [
        { target: RATE, duration: DURATION },
        { target: RATE * BURST, duration: BURST_DURATION },
      ],
    },
  },
};

const verdictDelay = new Trend('verdict_delay', true);

function pick() {
  let draw = Math.random() * TOTAL;
  for (const entry of CORPUS) {
    draw -= entry.weight;
    if (draw < 0) return entry;
  }
  return CORPUS[CORPUS.length - 1];
}

export default function () {
  const entry = pick();
  const started = Date.now();
  const submission = JSON.stringify({
    course: RUN,
    exercise: EXERCISE,
    language_pack: 'python',
    source: entry.source,
  });
  const submitted = http.post(`${API}/submissions`, submission, {
    headers: { 'Content-Type': 'application/json' },
    tags: { name: 'submit' },
  });
  if (!check(submitted, { accepted: (response) => response.status === 202 })) return;

  const id = submitted.json('id');
  while (Date.now() - started < WAIT * 1000) {
    sleep(POLL);
    const progress = http.get(`${API}/submissions/${id}`, { tags: { name: 'progress' } });
    const state = progress.status === 200 && progress.json('state');
    if (state === 'done' || state === 'failed') {
      verdictDelay.add(Date.now() - started, { source: entry.name });
      check(state, { judged: (state) => state === 'done' });
      return;
    }
  }
  check(id, { 'verdict before giving up': () => false });
}
