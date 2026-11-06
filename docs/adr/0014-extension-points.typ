#import "../template.typ": adr, arch, course, ext, validation

== ADR-0014 — Stable core and extension points <adr-0014>

*Status:* proposed. #adr("0012") and #adr("0013") apply it. \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("stable-core-and-extension-points")[Stable core and extension points].

=== Context

The platform starts with #course("LOG200"), then #course("LOG121"), and must eventually serve every LOG/GTI course and the DEG computing courses (see the #arch("appendix-course-inventory")[course inventory] in the architecture document). These courses differ in language (C89 to Kotlin, #ext("pep8")[Pep/8] assembly, #ext("oracle")[Oracle] SQL), in question format (code, multiple choice, short answer, Parsons problems, essays…, see #adr("0015")) and in activity mode (practice, assignment, exam). The project is maintained by one person: if each course requires changes in the core, the platform accumulates special cases until no one can change it safely. Conversely, an abstraction built for a need that never comes is also debt.

=== Options considered

- *Special cases in the core* (`if course == …`): fast for the first two courses, unmaintainable by the fifth.
- *A generic plug-in system for everything*: flexible on paper, but most interfaces would have a single implementation and would be designed without a real second case.
- *A small number of extension points, each justified by existing cases*: the core stays closed; everything that varies between courses goes through a contract.

=== Decision

Three rules:

+ *The core knows no special case.* No branch on a language, a course or an exercise identifier outside an extension point. The core handles opaque identifiers and contracts.
+ *An extension point is declarative data, a versioned contract and a conformance suite.* Adding an implementation means adding a directory and passing the conformance suite in CI. Implementations are discovered from their directory: there is no central list to edit.
+ *An extension point exists only if at least two real implementations are already known.* Anything else stays direct code, replaced when a second case appears.

#table(
  columns: (3.2cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Extension point*], [*Contract*], [*Initial implementations*],
  [Question type], [Item schema (public statement, private data), grader family, display component (#adr("0015"))], [The Q1 types: code, choice, key match, arrangement, essay],
  [Activity mode], [Policy: time window, feedback shown, check allowed (#adr("0040")), queue priority, SEB required, accommodations], [Practice, assignment, exam],
  [Language pack], [Image, compile and run commands, options, limits, capabilities (#adr("0013"))], [The P1 languages],
  [Test runner], [Exercise and artifact in, JSON report out (case, verdict code, message)], [Standard I/O, unit tests, SQL],
  [Judging service], [Provides a disposable environment to a runner, e.g. a database schema], [Ephemeral #ext("postgresql")[PostgreSQL], shared Oracle],
  [Enrollment source], [Produces (offering, user, role) triples (#adr("0012"))], [#ext("lti")[LTI 1.3] #ext("nrps")[Names and Roles], CSV import],
  [Isolation backend], [Starts a container with limits], [#ext("gvisor")[gVisor], #ext("firecracker")[Firecracker] (#adr("0002"))],
  [Statement renderer], [Already defined (#adr("0005"), #adr("0016"))], [Markdown, #ext("typst")[Typst], with Mermaid in both],
)

An exam or an assignment is a list of items of any type combined with a mode. A new exam format is a new mode or a new question type, never both at once. A course is an offering and a content repository: there is no per-course code.

*Public contracts are versioned*: verdict and error codes (already a contract under #adr("0011")), the runner report schema, the `exercise.json` schema and the language pack schema. Each schema carries a version; the core accepts versions N and N-1; a breaking change requires a new ADR.

*Guards against technical debt*:

- Component boundaries are checked in CI by an architecture test that reads the crate graph (#adr("0025")): the API does not depend on the judge, and the core does not depend on any implementation of an extension point.
- Every deliberate shortcut is marked in the code and collected by a report; an unmarked shortcut is a bug.
- Every contract has its ADR; an accepted ADR is superseded, never edited.
- Dependencies are pinned and language images are built in CI, never installed at run time.

=== Consequences

- Adding a course, a language or a question type is a content or configuration change reviewed like any other, without touching the core.
- LOG121 is the first test of this decision: any core change it requires is an architecture defect, recorded and fixed.
- Some variation that could be generalized stays hard-coded until a second case exists; this is intended.
- Conformance suites become part of the CI cost of every extension.

#validation(id: "V-0014")[
  When LOG121 is added, the diff touches only extension point implementations and content. The architecture test fails if the API depends on the judge or if the core depends on a language pack.
]
