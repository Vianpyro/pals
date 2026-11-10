#import "../template.typ": adr, arch, course, validation

== ADR-0043 — Contest mode <adr-0043>

*Status:* proposed. Refines #adr("0015"); applies #adr("0014") and #adr("0020"). \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("exam-mode")[Exam mode]; #adr("0005"), #adr("0012"), #adr("0021"), #adr("0040").

=== Context

The MVP serves one activity: the competitive programming sessions of #course("LOG200"). A session gives each student three problems to solve in two hours: one easy, one medium and one hard, each drawn from a bank. The problems are shown in random order, so their position says nothing about their difficulty. Students with an accommodation (ESH) get extra time.

The grade does not depend on which problem is solved, only on how many are fully solved: the first gives 50 %, the second 40 % more, the third the last 10 %. #adr("0015") grades a mixed assessment by the sum of its item scores, which cannot express this.

Until now an activity is a single exercise: a row of `schedules` names an exercise, and nothing groups three of them.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Contest in the content], [Reviewed and published with its exercises (#adr("0005")); one file serves every group and term (#adr("0012")).], [A change to the banks needs a release.],
  [Contest in the database], [Edited in the platform without a release.], [A second place where content lives; the release no longer says what a session holds.],
  [Grade by sum of item scores (#adr("0015"))], [Already decided; no new rule.], [Gives a grade per problem: solving the hard one would weigh more, which is not the rule.],
  [Grade by number of problems solved], [The rule of the course, whatever the draw.], [A rule specific to this mode.],
)

=== Decision

*A contest is an activity mode (#adr("0014")) and a file of the content*, `contests/<id>.toml`, published in the release. It holds exactly three slots, each a pool of exercise identifiers. A pool is not empty, an exercise belongs to at most one pool, and every exercise exists in the release. The `content` package refuses any other file.

*The draw is deterministic*, as in #adr("0015"): the server derives a seed from (offering, contest, student) and draws one exercise per slot, then the order in which the three are shown. A reload or a Safe Exam Browser restart shows the same problems in the same order. The seed uses a stable, documented hash: never Rust's `DefaultHasher`, whose algorithm may change between releases.

*A student reaches only their three exercises*, and only through the contest. The other exercises of the pools stay closed to them. The window is the contest's row in `schedules`, with the activity being the contest's identifier, and the single opening rule of #adr("0020") applies.

*Accommodations apply.* Each student's extra time is a percentage stored in the platform per offering and student, entered by staff, and applied by the opening rule (#adr("0020")). A student's drafts become final submissions at the close of their own window, not at the contest's row (#adr("0040")).

*A problem is solved* when the final job of its exercise has every verdict `passed`, hidden cases included. A partial result, an empty draft or no draft counts as unsolved.

*The grade is set by the number of problems solved*: 0, 1, 2 and 3 give 0, 50, 90 and 100 %. For the MVP, the three slots and this scale are fixed in the code. A contest's grade is computed only once every final job of the student is done; a job the judge failed is shown to staff and blocks that grade until it is judged again. Grades are released after the last accommodated window closes (#adr("0020")).

*Out of scope for the MVP.* A configurable number of slots and scale, written in the contest file. Penalties for time or failed attempts. A live ranking.

=== Consequences

- The scale and the number of problems need a code change until they are configurable; they move to the contest file when a second course needs another rule.
- Changing a contest's pools while a session is open draws other exercises for its students. Changes are published before the session; a guard is added if it happens.
- A pool of one exercise gives every student the same problem for that slot.
- A contest holds no date: groups get their own rows in `schedules` (#adr("0020")), and the judge reservation follows them (#adr("0021")).
- The accommodation table holds student data and must be included when an offering is purged (#adr("0012")).

#validation(id: "V-0043")[
  A contest file with two slots, an empty pool, an exercise in two pools, an unknown field or an unknown exercise is refused. The same student gets the same three exercises in the same order after a reload, two students can get different ones, and the draw is unchanged after a Rust update. A student cannot open an exercise of a pool they did not draw. A student with 50 % extra time can still save at 2 h 30 min, and their final submission is their draft at that instant. Final jobs with 0, 1, 2 and 3 problems fully passed give 0, 50, 90 and 100 %; a problem with one hidden case failed counts as unsolved, whichever slot it came from. No grade is shown while a final job is pending or failed, nor before the last accommodated window closes.
]
