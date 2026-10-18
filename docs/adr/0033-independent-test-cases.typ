#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0033 — Independent test cases and partial credit <adr-0033>

*Status:* proposed. Refines #adr("0008") and #adr("0013"); extends #adr("0015"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("judge-engine")[Judge engine] and #arch("load-and-performance")[Load and performance]; #adr("0007"), #adr("0022").

=== Context

A program that passes every test but the first must not get zero. Today:

- The standard I/O runner already judges every case, each in its own sandbox: a crash or a timeout marks that case and the next one runs. But each case pays a sandbox start, and a compiled language would pay one compilation per case, whereas #adr("0008") plans a single compilation per run.
- A unit-test runner (#ext("junit")[JUnit], #ext("pytest")[pytest], a C framework) runs the whole suite in one process. A segmentation fault, a `System.exit`, an `os._exit` or a stack overflow in one test kills the process: the report is lost, and every test with it.
- #adr("0008") stops hidden tests at the first failure. This is right for a test run, where the student only learns whether at least one hidden test fails, and wrong for grading.
- How an item's points follow from its case verdicts is not decided: #adr("0015") only says that a code item keeps its full verdict.

Running the cases in parallel was also suggested, to answer faster.

=== Options considered

- *Stop at the first failure*: cheapest, and the zero described above.
- *Cases in parallel inside a job*: faster when judges are idle. Under exam load every judge is already busy with other jobs (#adr("0013")); a sandbox has one CPU, so parallel cases take capacity from other students and do not raise throughput. Contention also makes wall-clock limits noisier.
- *Cases in sequence, each one isolated, in one sandbox per job*.

=== Decision

*One sandbox and one compilation per job.* The judge starts the job's sandbox, compiles once, then runs each case as a separate process inside it, with its own time and output limits. A crash or timeout ends that process and gives that case its verdict; the next case starts. Concurrency stays between jobs, as #adr("0013") sizes it.

*A unit-test suite resumes after a crash.* The runner writes one result line per test as it ends. If the process dies, the test in progress gets `crashed`, and the runner starts the suite again without the tests already done. A suite of $n$ tests restarts at most $n$ times, and a program that does not crash pays nothing extra.

#mermaid(
  "
  flowchart LR
    S[Start the suite<br/>without tests already done] --> T[Tests run,<br/>one result line each]
    T -->|process ends normally| R[Report]
    T -->|process dies| C[Test in progress: crashed]
    C --> S
  ",
  document-context: true,
  width: 100%,
)

*What runs depends on the job.*

#table(
  columns: (3.2cm, 1fr),
  stroke: 0.5pt,
  [*Job*], [*Cases run*],
  [Test run (#adr("0008"))], [Every visible case; hidden cases until the first failure, the rest marked `not-run`. What is shown is unchanged.],
  [Grading of the final submission (#adr("0022")), re-judging], [Every case, whatever happens to the others.],
)

*Partial credit.* In the exercise schema, each case has a weight, 1 by default. Cases may also be gathered in named groups, each with its own weight, that score all or nothing: a group counts only if every one of its cases passes. A case belongs to at most one group. A code item scores its points times the weight passed, ungrouped cases and groups together, over the total weight. A compilation error passes no case. A single group holding every case makes the item all or nothing.

*An infrastructure failure is not a case verdict.* If the sandbox itself cannot be started or is lost, the job is not graded: it is claimed again, as in #adr("0001").

=== Consequences

- The grade reflects the share of cases a program passes; one crash costs one case.
- A program that times out on every case costs the sum of the case limits. This is bounded by the exercise's limits, and the usage penalty of #adr("0022") charges it to its author.
- The per-case sandbox of the current standard I/O runner becomes one process per case inside the job's sandbox; the runner contract of #adr("0013") does not change.
- Each case's verdict is kept, so the weights can be changed and the item re-graded without running anything again.
- Parallel cases inside a job stay possible later, for deferred jobs on idle judges, if the measurements of #adr("0013") show that it helps.

#validation(id: "V-0033")[
  A program that crashes on the first case and passes the others gets every point but the first case's weight, in standard I/O and in each unit-test runner. A unit test calling `exit` or overflowing its stack loses that test only. A group with one failing case scores zero while the other cases still score. A C exercise compiles once per job. A test run stops hidden cases at the first failure; grading runs them all. Under the exam load test, compare the 95#super[th] percentile of run latency with one sandbox per case and one sandbox per job.
]
