#import "../template.typ": adr, arch, course, ext, mermaid, validation

== ADR-0032 — Find-and-fix exercises and author checkers <adr-0032>

*Status:* proposed. Extends #adr("0015"); applies #adr("0014") and #adr("0023"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("question-types")[Question types] and #arch("extensibility")[Extensibility]; #adr("0007"), #adr("0013").

=== Context

An instructor proposes an exercise type: the student receives a program with a defect (a bug, a performance problem, a missed edge case), writes tests that expose it, then fixes the program. The catalog of #adr("0015") already holds the two halves, in different tiers: "fix the bug" (Q2) and "tests written by the student, graded against mutants of a reference" (Q3, #course("LOG240")). What is missing is how the two combine, and what makes a student test count.

The instructor also asks, more generally, whether instructors could write plug-ins, in Python for example, and attach them to the platform. Two decisions already bound the answer: author code never runs in the publisher or the judge process (#adr("0023")), and an extension point exists only once two real implementations are known (#adr("0014")).

=== Options considered

- *A dedicated question type* with its own grader and display: duplicates the code and test families it is made of.
- *Plug-ins loaded by the platform*, in Python or another language: author code inside the API or the judge, next to every course's data; ruled out by #adr("0023").
- *Composition of existing items, and author checkers run in the sandbox*: the exercise is two items; anything the platform does not judge natively is an author program run like a test, which writes the runner's report.

=== Decision

*A find-and-fix exercise is two items.* The private part holds the reference solution $R$ and one or more defective programs $D_1, ..., D_n$; the public part shows $D_1$ as the template. The first item, of the student-tests family, receives the student's tests; the second, of the judged code family, receives the fixed program and is judged as any code item. To force the order, the instructor puts both in a section with sequential navigation (#adr("0015")). No new mechanism is needed.

*A student test counts when it is valid and reveals a defect.* Each test the student writes (standard input and expected output, or a unit test) is run in the sandbox against $R$ and each $D_i$:

- *valid*: $R$ passes it. A test that rejects the reference proves nothing.
- *reveals $D_i$*: valid, and $D_i$ fails it (wrong answer, crash or time limit).
- For a performance defect, revealing means that $D_i$'s instruction count exceeds the factor set by the exercise times $R$'s count on that input (#adr("0007")), never a wall-clock time, which varies with load.

The item's score is the share of defects revealed by at least one valid test. The number of tests per student and their size have limits in the exercise schema (#adr("0023")).

*The instructor decides what is checked during the activity.* For each exercise, or for a whole activity, the instructor chooses whether a test run shows, for each student test, its validity and the defects it reveals; whether it shows only whether at least one test is invalid, as for hidden tests (#adr("0008")); or whether test runs are disabled, so that the work is only judged at grading. These are settings of the feedback policy of the mode (#adr("0014")), exams included. Showing validity makes $R$ an oracle that the student can query; this is accepted as part of the exercise when the instructor enables it.

#mermaid(
  "
  flowchart LR
    T[Student test] --> RR{R passes?}
    RR -->|no| INV[Invalid]
    RR -->|yes| DD{D fails?<br/>or count above factor × R}
    DD -->|yes| REV[Reveals D]
    DD -->|no| VAL[Valid, reveals nothing]
  ",
  document-context: true,
  width: 100%,
)

*What instructors may plug in is a checker, run in the sandbox.* A checker is an author program in `assessment/`, written in any available language pack (Python included). It reads the case, the expected output and the student's output, and writes a verdict in the runner's JSON report (#adr("0013")). It covers what exact comparison cannot: floating-point tolerance, several valid answers, a property of the output. It runs in its own sandbox from its pack's image, so a Python checker can judge a Java exercise. It is author code, under the rules of #adr("0023"): charged to the offering, never run outside the sandbox, its report checked like any other.

*A new runner is not content.* A behavior that a checker cannot express (another test framework, a new judging service) is a runner: a directory in the platform repository with its image, its contract and its conformance suite, reviewed by the maintainer (#adr("0014")). A runner can be written in Python as well, since the judge only sees its JSON report. The student-tests family is the first runner added this way.

=== Consequences

- The find-and-fix exercise needs the student-tests runner and nothing else; the fix half works as soon as the code family does.
- Running each student test against $R$ and every $D_i$ multiplies the judge cost by $n + 1$ per test; the test limit and the offering's quota bound it (#adr("0022")).
- With validity shown, a student can check a guessed output against $R$ during an exam; each check is a test run, bounded by the usage penalty of #adr("0022").
- Disabling test runs applies to any code exercise, not only to find-and-fix: the feedback policy gains a "no test run" setting.
- Instructors get an extension of their own, the checker, without any author code in the platform's processes.

#validation(id: "V-0032")[
  A find-and-fix exercise with one bug and one performance defect is taken end to end: a test that rejects $R$ is invalid, a test that $D_1$ fails reveals it, and an input on which $D_2$ exceeds the instruction factor reveals $D_2$ identically on two runs. A Python checker with a floating-point tolerance judges a Java exercise. A checker that reads outside its exercise, or writes a report that does not match the schema, is refused as in #adr("0023").
]
