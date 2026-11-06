#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0030 — Run progress shown as a pipeline <adr-0030>

*Status:* proposed. Refines #adr("0001"); extends #adr("0008"); applies #adr("0014"). Refined by #adr("0040"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("exam-mode")[Exam mode] and #arch("load-and-performance")[Load and performance]; #adr("0019"), #adr("0022").

=== Context

Between clicking "precheck" or "check" (#adr("0040")) and the verdict, a run goes through the browser, the queue, the compiler and the tests (#adr("0008")). The student currently sees nothing until the verdict arrives. Under exam load the wait is longest, and a student who sees no progress clicks again or calls the invigilator. A failing run also does not say at once _where_ it failed: compilation, a visible test, a hidden test or a time limit. The first failing visible test is known long before the last test ends, and the student should not have to wait for it.

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
  [Tests], [Visible tests: expected and actual output per case, as soon as each case ends. Hidden tests, on a check only: passed, or at least one failed, with the run's verdict, as in #adr("0008"); never a count, a case name or a duration per case, whatever the mode.],
  [Verdict], [The run's verdict and total duration.],
)

Each step has an icon and a label as well as a color, and the current step is announced through an `aria-live` region (#ext("wcag")[WCAG 2.1 AA]). Beyond the hidden-test rule above, the detail shown follows the feedback policy of the activity mode (#adr("0014")). The student's past runs on an exercise are listed with their status, newest first.

*One event per step change, and one per visible case.* The job row records its current step; the judge writes it and sends `NOTIFY` when it claims the job, after compilation and with the verdict. That is two more writes per job than today, on a row the judge already updates. Step durations come from the timestamps of these writes.

*A visible case is reported as soon as it ends.* Before starting the next case, the judge commits the case's verdict and bounded output (#adr("0042")) as one row of a table of its own, keyed by job and position, and `NOTIFY` carries only the run, its author and the position. The API reads that row and sends one `case` event on the student's stream. The row is separate from the job's so that each write stays small, whatever the size of the source or of the earlier outputs.

*A hidden case is never reported.* Its verdict stays in the judge until the run's verdict: it has no row and no event, whatever the mode, so neither the table nor the stream can reveal it. A `final` job (#adr("0040")) reports nothing, since nobody is waiting for it.

*Events can be replayed.* A client that connects, reconnects or polls receives the current step of its run and every case reported so far. It keeps one result per position, so receiving a case twice changes nothing. A job reclaimed after a judge failure loses its rows in the transaction that puts it back in the queue, and the client clears the cases when the run goes back to waiting.

*What comes before the first result counts as much.* For a language retained by #adr("0008"), the first visible test ends in the browser without leaving it. For the others, the first result follows the wait, the sandbox start and the compilation. It therefore depends on two things this decision does not change: a judge woken by `NOTIFY` rather than by a poll (#adr("0001")), and one sandbox and one compilation per job (#adr("0033")). Reporting early does not shorten the wait in the queue.

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
- The judge writes the step twice more per job, and once per visible case of a test run that reaches the server. With ten visible cases and every language on the server, the assumptions of #arch("target-load")[Target load] give about 70 more commits per second at 1250 students, on top of about 500. Each one also sends a `NOTIFY`, which PostgreSQL serializes at commit. The load test measures this; if it limits, the judge groups the cases that end within a short delay, without changing the events.
- The time between the last visible case and the verdict tells the student how long the hidden cases ran in total. The total duration already gave an upper bound; this is accepted, since a check already reveals whether a hidden test fails, and an instructor who refuses this turns checks off (#adr("0040")).
- More notifications pass through the API's broadcast channel. A stream that falls too far behind ends, and the browser reconnects and receives the current state.
- The new table's rows are deleted with their job (#adr("0012")).
- Step labels go through the interface's translations (#adr("0011")).
- The final submission at the end of an exam is not concerned: it is graded later, with no one waiting (#adr("0022")).

#validation(id: "V-0030")[
  A double click produces one job. A run sent while the previous one is waiting leaves the previous one `replaced`, never claimed and not counted in usage. A judge stopped during a run shows the run as waiting again, then done. After an SSE disconnection, the client shows the current step and the cases reported so far, without duplicates, and polling shows the same. A program that fails its first visible case and sleeps on the second: the browser shows the failure before the second case ends. A reclaimed job clears its cases, then reports them again. In exam mode, no event sent to the browser contains a count, a name or a duration of hidden tests, and no row exists for a hidden case. A `final` job writes no case row. A screen reader announces each step change, and a run of ten fast cases stays readable with one. Under the exam load test, a test run produces at most three `NOTIFY` plus one per visible case; measure the 95#super[th] percentile of the delay between the click and the first result, per language, with idle judges and under load.
]
