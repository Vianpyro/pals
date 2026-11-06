#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0022 — Submission scheduling policy <adr-0022>

*Status:* proposed. Refines #adr("0001") and gives #adr("0014") its queue priority. Refined by #adr("0040"). \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("submission-queue")[Submission queue].

=== Context

The queue must support exam priority and a fair distribution of resources (#arch("submission-queue")[Submission queue]), but the order in which judges claim jobs is not decided (#arch("open-questions")[open question 2]). Three situations must be handled:

- During an exam, an exam job must not wait behind practice jobs. The reserved judges (#adr("0021")) guarantee capacity, not order.
- With strict priority, a low-priority activity (practice, bonus, the measurement queue of #adr("0007")) can wait forever while higher-priority jobs keep arriving.
- One student who submits long runs again and again can hold every judge, and others wait behind them.
- When an exam or a graded lab closes, the platform itself queues a burst: grading of every final submission (#adr("0008")), then performance measurement of the final submission of each student for each exercise (#adr("0007")). Nobody waits for it on screen, but the instructor needs it to release grades, and it is a requirement of the exercise, not a student's behavior.

A slow program is not misbehavior: a legitimate algorithm may take 20 s. The policy must react to the load a person keeps imposing, not to the duration of one run. New activity modes (lab, quiz, contest) will be added under #adr("0014"), and adding one must not require retuning the others.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [FIFO], [Simple, no starvation.], [No exam priority; one person can fill the queue.],
  [Strict priority per mode], [Exams always first.], [Lower modes can starve; no fairness between people.],
  [Weighted fair queuing per person], [Fair between people.], [Ignores the activity mode; virtual-time state to maintain.],
  [Protected modes, spaced base priority, aging, per-person usage penalty],
  [Exams first; no starvation; fairness between people; expressed in the claim query.],
  [Parameters to tune by load testing.],

  [Central scheduler with an in-memory ordered structure (red-black tree, B-tree, heap)],
  [Efficient range queries and reordering.],
  [Adds a stateful dispatcher, against #adr("0001") and #adr("0021"), where judges pull from the queue; the waiting queue holds a few dozen jobs (#arch("target-load")[Target load]), so ordering costs nothing next to a compilation.],
)

=== Decision

Judges keep claiming with `SELECT … FOR UPDATE SKIP LOCKED` (#adr("0001")). The claim query orders waiting jobs by:

```
(protected DESC, base + aging - penalty DESC, enqueued_at, id)
```

#mermaid(
  "
  flowchart TD
    W[Waiting jobs] --> K1[1. protected first<br/>exam]
    K1 --> K2[2. highest score]
    K2 --> K3[3. oldest enqueued_at]
    K3 --> K4[4. lowest id]
    K4 --> C[Claimed with<br/>FOR UPDATE SKIP LOCKED]
    subgraph SCORE[Score]
      B[base<br/>mode or kind]
      A[+ aging<br/>10 per minute, up to 300]
      P[- penalty<br/>k × usage, up to 300]
    end
    SCORE --> K2
  ",
  document-context: true,
)

- *Protected modes*: a mode is `protected` or not; exam is protected. A protected job is always claimed before any other, whatever its score. This is a flag, not a number, so no aging or penalty setting can move a non-exam job ahead of an exam.
- *Live and deferred jobs*: a _live_ job has someone waiting for its answer (a precheck or a check, #adr("0040")) and takes the base of its activity mode. A _deferred_ job is queued by the platform: grading of final submissions when a session closes (#adr("0008")), performance measurement (#adr("0007")), and re-judging requested by an instructor (#adr("0012")). A deferred job is never protected, so grading an exam that has ended never goes ahead of the test runs of an exam in progress, and it is never charged to anyone's usage.
- *Base priority*: an integer, not code. For a live job it is part of the activity mode policy (#adr("0014")); for a deferred job it is set per kind. Values are spaced so that a new mode or kind takes a free value between two existing ones, without renumbering or retuning anything:

  #table(
    columns: (1fr, 2cm, 2cm),
    stroke: 0.5pt,
    [*Mode or kind*], [*Job*], [*Base*],
    [Exam (protected)], [Live], [1000],
    [Assignment], [Live], [700],
    [Final grading (#adr("0008"))], [Deferred], [600],
    [Practice], [Live], [400],
    [Re-judging], [Deferred], [250],
    [Bonus], [Live], [100],
    [Performance measurement (#adr("0007"))], [Deferred], [0],
  )

  A lab mode added later, for example, would take 550.
- *Aging and penalty in absolute points*: aging adds a fixed number of points per minute of waiting, up to a cap; the penalty removes up to a cap. Neither depends on the gap between two modes, so adding a mode does not change their effect. Initial values: +10 points per minute, capped at +300; penalty capped at 300. A lower mode can therefore overtake a higher one only if they are less than 300 points apart and after a long wait, which is the protection against starvation.
- *Fairness between people*: each person has a limit of running jobs; among waiting jobs, the claim favors the person with the fewest running jobs, then their oldest job. The per-offering quota of #adr("0012") applies on top.
- *Usage penalty*: each person has a usage that decays exponentially, as in #ext("slurm-fairshare")[Slurm fair-share]. On each verdict, `usage = usage × 2^(-Δt / half_life) + contended × judge_seconds`, and `penalty = min(cap, k × usage)`. It is one column in #ext("postgresql")[PostgreSQL].
  - `contended` is 1 if a live job was claimed while at least one job from another person was waiting, and 0 otherwise; it is always 0 for a deferred job. It is one `EXISTS` in the claim transaction. Work done while nobody waits delays nobody and does not count; a person's own waiting jobs do not count either.
  - The decay depends on the clock, not on submissions: without contention, every usage returns to 0 by itself. A quiet period never raises it.
  - The penalty is bounded and cannot be saved up in advance; it grows with repeated load imposed on others, not with one slow run. Submission rate is already bounded by the running-job limit and by #ext("nginx-limit-req")[`limit_req`].
- *Grade turnaround*: how fast deferred work finishes is a matter of capacity, not priority. Waiting deferred jobs make the scaler start on-demand judges up to the cap (#adr("0021")). Since measurement is deterministic, it can also run on another VM without skewing the result (#adr("0007")). A deadline-based boost is only added if load tests show the turnaround is not met.
- *Parameters* (base values, aging rate and cap, half-life, `k`, penalty cap, running-job limit) are read at claim time, so changing them needs no redeployment. Load tests set them.

An exam or a graded lab closes at its last student's window, accommodations included (#adr("0020")); the deferred work then follows in this order:

#mermaid(
  "
  flowchart LR
    E[Session closes] --> G[Grading of every final submission<br/>deferred, base 600]
    G --> M[Performance measurement of the final<br/>submission per student and exercise<br/>deferred, base 0]
    M --> R[Grades available]
    R --> AGS[Moodle, through AGS]
  ",
  document-context: true,
  width: 100%,
)

No scheduler process is added: the policy is the claim query.

=== Consequences

- Adding an activity mode means choosing a base value and whether it is protected; existing modes are untouched.
- The dashboard (#adr("0019")) shows waiting time per mode and the usage penalty per person, so a long wait can be explained to a student. For each closed session, it shows the progress of grading and measurement.
- Grades reach Moodle through #ext("ags")[AGS] once deferred work is done, possibly after the session has closed (#arch("open-questions")[open question 22]).
- During a quiet period the penalty changes nothing anyway, since nobody waits. Counting only contended work means a student who works at night does not carry a penalty into the next peak. On-demand judges (#adr("0021")) do not change this: as soon as a job waits, work counts.
- The usage is personal data: it is purged with the offering (#adr("0012")).
- The claim query computes a score over every waiting job. This is cheap at the expected queue sizes; an index on `(protected, base, enqueued_at)` keeps it so if the queue grows.

#validation(id: "V-0022")[
  Under a simulated exam load with one student flooding the queue, compare FIFO, strict priority, aging alone and this policy. Measure the p95 waiting time of exam jobs, the maximum waiting time of bonus jobs, and the waiting time of the other students with and without the flooding student. A student who submits heavily during a quiet period starts the next peak with zero usage. After a 250-student exam, measure the time until every grade is available (grading and measurement), with and without another session in progress; no deferred job raises a student's usage.
]
