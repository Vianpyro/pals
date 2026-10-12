#import "../template.typ": arch, ext, mermaid, validation

== ADR-0001 — Submission queue in PostgreSQL <adr-0001>

*Status:* accepted, pending validation by load testing. \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("submission-queue")[Submission queue].

=== Context

The application and the judge engine must be decoupled by a queue able to absorb the peaks of an exam (about 400 students), with priorities, controlled retries and detection of abandoned jobs. The platform is deployed on a single VM with limited resources, where every added service takes memory and CPU away from judging.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [#ext("postgresql")[PostgreSQL] (#ext("skip-locked")[`SKIP LOCKED`], #ext("listen-notify")[`LISTEN/NOTIFY`])],
  [No added service; queue and submission state in the same transaction; priorities and retries expressed in SQL.],
  [Throughput capped by the database; not designed as a broker.],

  [#ext("redis")[Redis]], [Fast, simple.], [One more service; persistence and transactions separate from the state.],
  [#ext("rabbitmq")[RabbitMQ]], [Complete queue semantics.], [Heavy service to operate for a single VM.],
  [Spool directory (#ext("ctester")[CTester])],
  [No dependency; proven in production.],
  [Single host; scheduling and fairness to be rewritten by hand.],
)

=== Decision

The queue is implemented in PostgreSQL. Judges pull jobs with `SELECT … FOR UPDATE SKIP LOCKED` and are woken up by `LISTEN/NOTIFY`.

#mermaid(
  "
  stateDiagram-v2
    direction LR
    [*] --> waiting: inserted with the submission, NOTIFY
    waiting --> running: claimed with SKIP LOCKED
    running --> done: verdict written with the pedagogical state
    running --> waiting: abandoned by a failed judge, reclaimed
    running --> failed: cannot be judged (language pack, exercise, cases)
    done --> [*]
    failed --> [*]
  ",
  document-context: true,
  width: 100%,
)

=== Consequences

- The queue acts as the dispatcher: capacity is adjusted through the number of judges, without a load balancer.
- A job and its verdict are written atomically with the pedagogical state.
- A job the judge cannot judge (unsupported language pack, missing exercise, invalid cases) ends `failed` and is never reclaimed. An infrastructure failure (sandbox runtime) stops the judge instead, so a broken judge cannot fail the whole queue.
- Judges need access to the database. Their privileges must be restricted to the queue tables (see the threat model).

#validation(id: "V-0001")[
  Measure queue throughput and latency under a simulated exam load. A dedicated broker is only considered if a limit is measured.
]
