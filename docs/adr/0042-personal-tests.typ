#import "../template.typ": adr, arch, ext, validation

== ADR-0042 — Personal tests <adr-0042>

*Status:* proposed. Extends #adr("0008") and #adr("0040"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("exam-mode")[Exam mode] and #arch("load-and-performance")[Load and performance]; #adr("0012"), #adr("0013"), #adr("0022"), #adr("0023"), #adr("0031"), #adr("0032"), #adr("0033"), #adr("0038"), #adr("0039"), #adr("0041").

=== Context

A student sees how their program behaves on the cases of the exercise, and on nothing else. To try an edge case of their own, they must change their code to read a fixed value and remember to undo it. Under #ext("seb")[Safe Exam Browser], they have nowhere else to run it.

The student-tests family of #adr("0032") does not answer this. Its tests are graded against a reference and defective programs, and the exercise must be written for it. What is missing here is a tool for any standard I/O exercise, which plays no part in the grade.

Such a test needs a place to live. The draft of #adr("0038") looks like the obvious one, since it is already saved per student and item and comes back after a restart of SEB.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [In the browser only], [Nothing to store on the server.], [Lost on another workstation, and left behind on a shared one. Unavailable to a job for the languages that run on the server.],
  [A field of the draft], [No new table; revisions, conflicts and the encrypted buffer come for free.], [The draft is the final submission (#adr("0040")) and holds one language pack (#adr("0041")). A test edited in one tab causes a `409` on the code in another. Every save of the code sends the tests too, and they share its 1 MiB.],
  [One document per student and item, with its own revision], [One write for everything; the same protocol as drafts.], [A conflict interface for data that is not graded, and the whole document sent on each write.],
  [One row per test], [Each test is written alone and idempotently, with its own limits and no conflict.], [A table and three routes. Two tabs editing the same test: the last one wins.],
)

=== Decision

*A personal test is an input and an expected output*, of the same form as a case of the exercise and compared by the same rule. It is never graded. It only runs the student's own code, so it reveals nothing about hidden tests. Only exercises judged by the standard I/O runner (#adr("0013")) offer personal tests.

*One row per test, in a table of its own:* `personal_tests`, keyed by author, course, exercise and id. A test holds no language, so it survives a change of language pack. The author is part of the key, so a student cannot overwrite another's test, even with the same id.

*The API has three routes*, which go through the gate of #adr("0020") like drafts: list, `PUT` and `DELETE`. The browser generates the id, so a `PUT` sent again after a lost acknowledgement creates no duplicate. An exercise holds at most 10 tests per student, about as many as its visible tests (#adr("0008")). Input and expected output are limited to 64 KiB each. The API answers `422` beyond these limits and for a NUL byte, which PostgreSQL's `text` refuses.

*No revision: the last write wins.* This departs on purpose from "no write is lost without the student seeing it" (#adr("0038")): that rule protects what is graded, and a conflict interface during an exam is a cost that a personal test does not justify.

*A personal test runs with the precheck* (#adr("0040")), wherever a precheck is available, exams included:

- For a language retained by #adr("0008"), the browser runs the personal tests after the visible ones, and no job starts.
- For another language, the API copies the student's tests into the `precheck` job at enqueue time. The judge therefore runs the tests as they were at the click, and has no access to the table. The contract of a submission does not change. A `check` or `final` job cannot hold any: a database constraint enforces it.
- An exercise without a visible case offers a precheck as soon as the student has a personal test, and it runs only those. The "no test run" setting of #adr("0032") turns everything off.

*A personal test has no effect on any other result.* Its verdicts are stored apart from those of the exercise's cases. A failure changes neither the run's overall result, nor the draft saved by a passing precheck, nor access to the check.

*The judge runs them in the same sandbox*, after the exercise's cases and with the same compilation and limits (#adr("0033")). Each gets its verdict like any case, and is reported as soon as it ends (#adr("0030")).

*The stored output is bounded and cleaned*, for every case that shows one. The verdict is computed on the full output first. The output is then cut to 64 KiB, and the interface says so. A NUL byte or invalid UTF-8 is replaced by U+FFFD, since `jsonb` refuses them and a database error stops the judge: otherwise, a program printing `\0` could stop it.

*The student sees the input, the expected output and the actual output*, as text and never as HTML (#adr("0023")).

*Out of scope.* Staff do not see personal tests, and the assistant of #adr("0031") does not receive them. The encrypted buffer of #adr("0038") does not hold them: a test is saved by an explicit action, and when that fails, its text stays on screen with the failure.

=== Consequences

- A test not yet saved is lost if the network drops and SEB restarts. This is revisited if it happens during an exam.
- Up to 10 tests that each reach the 10 s limit add 100 s to a job, for the languages that run on the server. The usage penalty (#adr("0022")) and the single run per student (#adr("0040")) limit this, and the sizing of #adr("0039") must measure it.
- A job can hold up to 1.28 MiB of personal tests, and its result 640 KiB of their outputs.
- Two simultaneous writes can exceed the limit of 10 tests. This does no harm: the API copies at most 10 into a job.
- For a language retained by #adr("0008"), a personal test only runs in the browser, so its result is never compared with the judge's. The server remains the reference for the exercise's cases only.
- The table holds student data and must be included when an offering is purged (#adr("0012")).
- A precheck can now reach the queue for an exercise without a visible case.
- The labels go through the interface's translations (#adr("0011")).

#validation(id: "V-0042")[
  A failing personal test leaves `passed` true and does not block a check. A `check` or `final` job with a personal test is refused by the database. Student B can neither read nor overwrite student A's tests, even with the same id, and the judge's role has no access to the table. Changing a test after the click does not change the run in progress. A repeated `PUT` creates no duplicate, and a repeated `DELETE` succeeds. Tests come back after a change of language pack and a restart of SEB, and editing them causes no `409` on the draft. A program that prints a NUL byte, invalid UTF-8 or 16 MiB gets a normal verdict and a bounded output, and the judge keeps running. An eleventh test, an input of 64 KiB + 1 and a NUL byte get `422`. On a closed exercise, the gate refuses all three routes. Adding, running and reading a test works by keyboard and passes an accessibility check. Under the exam load test, compare the 95#super[th] percentile of precheck latency with and without 10 personal tests; if the gap is large, lower the limit.
]
