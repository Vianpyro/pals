#import "../template.typ": adr, arch, ext, validation

== ADR-0040 — Precheck and check <adr-0040>

*Status:* proposed. Refines #adr("0008"), #adr("0022"), #adr("0030") and #adr("0033"); extends #adr("0038"); applies #adr("0014"). Extended by #adr("0042"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("exam-mode")[Exam mode] and #arch("load-and-performance")[Load and performance]; #adr("0007"), #adr("0020"), #adr("0032").

=== Context

#adr("0008") gives the student one "test" button: it runs the visible tests, then the hidden ones until the first failure. Every click that passes the visible tests takes a place in the queue, and when the student only wants to know about the visible tests, the server runs the hidden ones anyway. During an exam, each click also tells the student whether a hidden test fails, which they can use to probe those tests.

Students already know two separate actions from #ext("coderunner")[CodeRunner] in #ext("moodle")[Moodle]: "Précontrôle" (precheck) and "Vérifier" (check). What the final submission is, and when its grade appears, is not written down either: #adr("0008") only says that it happens automatically at the end of the exam.

=== Options considered

- *One button* (#adr("0008")): one path, but a queue place for each click that passes the visible tests, and hidden tests can always be probed.
- *Hidden tests at the end only*: nothing to probe and less load, but a student never learns about a hidden test during the activity, even in practice.
- *Two actions, the check being a setting of the activity*: the instructor chooses between the two previous behaviors.

=== Decision

*A test run is a precheck or a check.* The job's kind decides which cases run, and the judge applies it:

#table(
  columns: (2.6cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Action*], [*Cases run*], [*Shown to the student*],
  [Precheck], [Every visible case: in the browser for a language retained by #adr("0008"), otherwise as a `precheck` job.], [Expected and actual output per case, as soon as each case ends (#adr("0030")).],
  [Check], [Every visible case; hidden cases until the first failure, and none if a visible case fails. The cases left are marked `not-run` (#adr("0033")). A `check` job.], [Visible cases as above; for hidden cases, only whether at least one fails.],
  [Final submission], [Every case. A deferred `final` job (#adr("0022")).], [Nothing on screen; the grade, according to the activity.],
)

*A check always passes the visible tests first, without a second click.* For a retained language, the browser runs them and sends nothing if one fails. The judge applies the same rule: a request that bypasses the browser learns nothing about hidden tests while a visible test fails.

*A precheck in the browser starts no job.* When it passes, the draft is sent at once through the protocol of #adr("0038"), with its revisions and conflicts. A discrepancy between the browser and the judge therefore appears on a check.

*The instructor decides per activity.* Whether a check is allowed is a setting of the mode's feedback policy (#adr("0014")), exams included, and the API refuses a `check` job when it is not. The "no test run" setting of #adr("0032") turns off both actions. An exercise without a visible case offers no precheck: only a check when allowed, otherwise it is judged at the final submission.

*The final submission is the last saved draft.* It is the server's draft when the student's window closes (#adr("0020")), grace period included (#adr("0038")). No button triggers grading, and no check is needed: a student edits until the close. An unresolved conflict leaves the server's revision, and an item without a draft scores zero without a job. Performance measurement then uses this submission (#adr("0007")). A practice activity has no final submission: its check is its feedback.

*The grade appears after the final submission, and only if the instructor allows it.* This is part of the mode's feedback policy, and it waits for the end of the session as in #adr("0020").

*Clicks cost no points and have no limit.* The only cost is the usage penalty of #adr("0022"), which acts on the order in the queue. Both kinds take the base priority of the activity: a check takes longer because it runs more cases, not because it waits behind a precheck. A student keeps one run in progress, and a new run of either kind replaces a waiting one (#adr("0030")).

=== Consequences

- Allowing checks during an exam allows probing hidden tests, bounded by the wait and not by the grade. An instructor who refuses this turns checks off.
- With checks off, a student who passes every visible test learns about hidden tests only with the grade, and a discrepancy between the browser and the judge appears only at grading. The visible tests must therefore represent the exercise, and V-0008 must show few discrepancies.
- Prechecks in the browser never reach the queue. Those of other languages do, and count in usage like any live job. The sizing of #adr("0039") must separate the three kinds, including the burst of `final` jobs at the close.
- `jobs` gains a kind, read by the judge. A `not-run` case always follows a failure, so a run's overall result does not change.
- The labels go through the interface's translations (#adr("0011")).

#validation(id: "V-0040")[
  On an exercise of two visible and three hidden cases, a precheck job gives two verdicts and a final job gives five, whatever fails. A check sent straight to the API with a failing visible case runs no hidden case. A check whose first hidden case fails marks the others `not-run` and shows only that at least one fails. A precheck that passes in the browser writes the draft and creates no job. With checks off, the API refuses a `check` job and the button is absent. A student who never clicks anything is graded on their last draft. A grade is shown neither before the close nor without the instructor's permission. Under the exam load test, measure the share of clicks that reach the queue and the 95#super[th] percentile of precheck and check latency.
]
