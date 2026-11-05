#import "../template.typ": adr, arch, course, ext, mermaid, validation

== ADR-0015 — Question types and mixed assessments <adr-0015>

*Status:* proposed. Applies #adr("0014"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("question-types")[Question types] and #arch("mixed-assessments")[Mixed assessments].

=== Context

Instructors expect at least every #ext("moodle-questions")[Moodle question type] (multiple choice, true/false, matching, short answer, numerical, calculated, essay, drag and drop, embedded answers…), plus what a programming course needs and Moodle only offers through plug-ins such as #ext("coderunner")[CodeRunner]: code judged by tests, #ext("parsons")[Parsons problems], predicting a program's output, finding a bug. An exam must be able to mix all of them: a few multiple-choice questions, a Parsons problem and two code exercises in the same session, under #ext("seb")[Safe Exam Browser]. Implementing some thirty types one by one is out of reach for one person, and delegating the non-code questions to a Moodle quiz would split one exam across two systems, two timers and two SEB configurations.

=== Options considered

- *One implementation per type*, as Moodle does: thirty graders, thirty display components, each with its own bugs.
- *Non-code questions in a Moodle quiz, code in the platform*: two exams to synchronize, and Moodle may be unavailable (project plan, risk R6).
- *A few grader families*: a question type is a declarative item schema bound to one of a small number of graders; most types differ only by their schema and display.

=== Decision

*Items.* Every question is an item with a public part (stem, choices, templates) and a private part (key, tolerances, tests), published like an exercise (#adr("0005")). Every text field (stem, choices, feedback, hints) goes through the statement renderers: Markdown or #ext("typst")[Typst], with #ext("mermaid")[Mermaid] diagrams in both (#adr("0016")).

*Catalog.* Types are grouped by grader family:

#table(
  columns: (2.8cm, 1fr, 1cm),
  stroke: 0.5pt,
  [*Family*], [*Types*], [*Tier*],
  [Choice], [Single choice (radio), true/false, multiple choice (check boxes, partial credit, penalties), dropdown select], [Q1],
  [], [Grid / Kprim (true or false per row)], [Q2],
  [Key match], [Short answer (exact, case-insensitive, wildcards), numerical (tolerance, units)], [Q1],
  [], [Regular expression answer, calculated (random parameters and formula), calculated multiple choice], [Q2],
  [Arrangement], [Matching, ordering / ranking], [Q1],
  [], [Random short-answer matching, categorization, drag and drop into text, select missing words, Parsons problem (reorder code lines, with distractors)], [Q2],
  [Composite], [Embedded answers (cloze): sub-items of any family inline in a text], [Q2],
  [Judged code], [Full program (standard I/O), function or class (unit tests) (#adr("0013"))], [Q1],
  [], [Fill-in-the-blanks code, fix the bug, SQL query], [Q2],
  [], [Tests written by the student, graded against mutants of a reference (#course("LOG240"))], [Q3],
  [Derived key], [Predict the output or trace a program, choose the complexity: the key is computed by running the reference at publication time, never typed by hand], [Q2],
  [Area selection], [Hotspot on an image, drag markers or labels onto an image, click the faulty line(s) of a code listing], [Q3],
  [Math expression], [Symbolic answer checked for equivalence, in the manner of #ext("stack")[STACK]], [Q3],
  [Manual], [Essay / long answer], [Q1],
  [], [File upload, diagram (UML, #course("LOG121")), code review annotation: graded with a rubric by a TA or an instructor], [Q3],
  [Ungraded], [Description / information block, random question (draw from a pool)], [Q1],
  [], [Survey / Likert scale], [Q3],
)

Tiers are an order, not an exclusion: Q1 comes with #course("LOG200") and #course("LOG121"), Q2 after load validation, Q3 on request. Audio and video recording are out of scope.

*Mixed assessments.* An exam or assignment is an activity mode (#adr("0014")) and an ordered list of _slots_. A slot is a fixed item or a draw of $n$ items from a pool (a tag or a category of the course bank), with its points, and whether its choices are shuffled. The exam defines its sections, whether slots are shuffled and whether navigation is free or sequential. Draws, shuffles and calculated variants are deterministic: the seed is derived from (offering, exam, student) by the server, so a reload or a Safe Exam Browser restart shows the same exam. Every item is autosaved as a draft. The score is the sum of item scores; a code item keeps its full judge verdict.

#mermaid(
  "
  classDiagram
    direction LR
    class Exam {
      activity mode
      sections
      shuffled slots or not
      free or sequential navigation
    }
    class Slot {
      points
      shuffled choices or not
    }
    class FixedItem
    class PoolDraw {
      n items
      pool: tag or category
    }
    class Item {
      public part
      private part
      grader family
    }
    Exam \"1\" *-- \"many\" Slot
    Slot <|-- FixedItem
    Slot <|-- PoolDraw
    FixedItem --> \"1\" Item
    PoolDraw --> \"n\" Item : seeded by offering, exam, student
  ",
  document-context: true,
  width: 100%,
)

*Grading.* The judge stays the only reader of assessment data (#arch("double-check-by-the-judge")[architecture]). Non-code graders are pure functions over data and run in the judge process, without a sandbox; code items go through the sandbox as before. Regular expressions written by instructors run on a linear-time engine (#ext("re2")[RE2]), which rules out catastrophic backtracking. Calculated variants and derived keys are computed at publication time, never per request. Manual items wait in a grading queue visible to the TAs of the offering (#adr("0012")).

*Answer shape.* The interface may say that an answer does not have the expected shape (eight binary digits, a number, a unit), never whether it is right: a per-field right/wrong hint would let an 8-bit answer be found in 256 tries. When the same rule is written twice, once in the browser and once in the grader, one shared file of test vectors binds both implementations, as in #ext("ctester")[CTester].

*Projection.* The keys reserved for assessment data grow with the families (`correct`, `key`, `tolerance`, `pairs`, `order`, `regions`, private feedback…) and stay checked by negative enumeration (#adr("0005")).

=== Consequences

- About nine graders and their display components cover some thirty types; a new type usually adds a schema and a display, not a grader.
- One exam, one timer, one SEB configuration, whatever the mix of items. Moodle only receives the grade (#ext("ags")[AGS]).
- Grading non-code items in the judge adds a round trip through the queue for a trivial computation; exam priority keeps it short.
- Importing existing Moodle banks (Moodle XML, GIFT) is not part of this decision; see #arch("open-questions")[open question 19].
- Within Q2, Parsons problems come last: LOG200 students rate them the least useful type (21 of 47 "not useful"), and predicting the output the most useful (#arch("appendix-student-survey")[student survey]).

#validation(id: "V-0015")[
  One exam mixing at least one item of each Q1 family is taken end to end under SEB and graded correctly. After a SEB restart, the student gets the same draw, order and variants. The published projection of a bank contains no key of any family. A regular expression known to backtrack catastrophically is evaluated in bounded time.
]
