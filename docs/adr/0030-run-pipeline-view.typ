#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0030 — Run progress shown as a pipeline <adr-0030>

*Status:* proposed. Refines #adr("0001"); extends #adr("0008"); applies #adr("0014"). Refined by #adr("0040"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("exam-mode")[Exam mode] and #arch("load-and-performance")[Load and performance]; #adr("0019"), #adr("0022").

=== Context

Between clicking "precheck" or "check" (#adr("0040")) and the verdict, a run goes through the browser, the queue, the compiler and the tests (#adr("0008")). The student currently sees nothing until the verdict arrives. Under exam load the wait is longest, and a student who sees no progress clicks again or calls the invigilator. A failing run also does not say at once _where_ it failed: compilation, a visible test, a hidden test or a time limit.

Students know the step-by-step view of #ext("github-actions")[GitHub Actions]: one line per step, each with a status and a duration, and a list of past runs. The platform already holds the data for such a view: the job's queue state (#adr("0001")), the single compilation and the test results of the runner's JSON report (#adr("0013")), and a server-sent event stream per student (#adr("0008")).

=== Options considered

- *Verdict only*: nothing to build; the wait and the cause of a failure stay opaque.
- *Workflow view as in GitHub Actions*: configurable jobs and steps, raw logs. Exercises have no workflow to configure, and raw logs of hidden tests would expose assessment data.
- *Fixed steps*: the same short sequence for every run, derived from states the platform already has.

=== Decision

*Fixed steps.* Each run is shown as a sequence of steps. A step a run does not have is not shown.

#table(
  columns: (3.4cm, 1fr),
  stroke: 0.5pt,
  [*Step*], [*Shown*],
  [Browser tests], [Results of the visible tests as they come in, for languages run in the browser (#adr("0008")).],
  [Waiting], [Elapsed time. No position: the claim order depends on priority and usage (#adr("0022")), and a position that moves backwards misleads more than it helps.],
  [Compilation], [Compiler diagnostics. Omitted when the language pack has no compile command (#adr("0013")).],
  [Tests], [Visible tests: expected and actual output per case. Hidden tests, on a check only: passed, or at least one failed, as in #adr("0008"); never a count, a case name or a duration per case, whatever the mode.],
  [Verdict], [The run's verdict and total duration.],
)

Each step has an icon and a label as well as a color, and the current step is announced through an `aria-live` region (#ext("wcag")[WCAG 2.1 AA]). Beyond the hidden-test rule above, the detail shown follows the feedback policy of the activity mode (#adr("0014")). The student's past runs on an exercise are listed with their status, newest first.

*One event per step change, not per test case.* The job row records its current step; the judge writes it and sends `NOTIFY` when it claims the job, after compilation and with the verdict. That is two more writes per job than today, on a row the judge already updates. Step durations come from the timestamps of these writes. A client reconnecting with `Last-Event-ID` receives the current step of its run.

*Clicking again replaces, nothing cancels.* As in #adr("0008"), a student has at most one run in progress, and a new run replaces the previous one while it is still waiting. The replaced job ends in a new terminal state, `replaced`, written in the same transaction as the new job. It is never claimed and not charged to the student's usage (#adr("0022")). Its code stays in the history.

#mermaid(
  "
  stateDiagram-v2
    direction LR
    [*] --> waiting
    waiting --> replaced: newer run by the same student
    waiting --> running: claimed
    running --> waiting: reclaimed, shown as waiting again
    running --> done
    replaced --> [*]
    done --> [*]
  ",
  document-context: true,
  width: 100%,
)

A run already running is not stopped: it lasts a few seconds, and stopping it would require signalling the judge. It ends and shows its own verdict in the list, under the newer run. No separate "cancel" button exists, since it would do what clicking again does, without new code. A job reclaimed after a judge failure (#adr("0001")) goes back to the waiting step with a note, so that the student does not click again.

=== Consequences

- The queue gains the `replaced` state and a step column; the dashboard (#adr("0019")) shows replaced jobs like the others.
- The judge writes the step twice more per job. Under the exam load of #arch("load-and-performance")[Load and performance], this adds a few writes per second at most, to be confirmed by the load test of #adr("0008").
- Step labels go through the interface's translations (#adr("0011")).
- The final submission at the end of an exam is not concerned: it is graded later, with no one waiting (#adr("0022")).

#validation(id: "V-0030")[
  A double click produces one job. A run sent while the previous one is waiting leaves the previous one `replaced`, never claimed and not counted in usage. A judge stopped during a run shows the run as waiting again, then done. After an SSE disconnection, the client shows the current step. In exam mode, no event sent to the browser contains a count, a name or a duration of hidden tests. A screen reader announces each step change. Under the exam load test, each job produces at most three `NOTIFY`.
]
