#import "../template.typ": adr, arch, course, ext, validation

== ADR-0041 — Exercises with several source files <adr-0041>

*Status:* proposed. Extends #adr("0008"), #adr("0013"), #adr("0034"), #adr("0035") and #adr("0038"); applies #adr("0023"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("validation")[Validation] and #arch("safe-exam-browser")[Safe Exam Browser]; #adr("0015"), #adr("0020"), #adr("0032"), #adr("0033"), #adr("0040").

=== Context

Nothing in the documentation says whether a code item may span several files, and the implementation assumes one: submissions and drafts hold a single `source`, a language pack names a single file, the judge writes that file and the interface edits it.

Standard I/O exercises for #course("LOG200") do not need more. #course("LOG121") teaches object-oriented design in Java and is the first course likely to: several classes can share one `Main.java`, but splitting a program into files and packages is then out of reach, and an exercise cannot hand out an interface that the student must implement without being able to change it. C needs several files for its headers. No coordinator has asked for this yet (#arch("open-questions")[open question 27]); it is decided now because it changes the submission and draft contracts, which are cheaper to change before real data exists.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [One file only], [Nothing to build.], [No real split into files; no headers in C; a provided file can only be pasted into the statement.],
  [Archive upload], [Students work in their own IDE.], [#ext("seb")[Safe Exam Browser] restricts the local disk; archive bombs and symbolic links to handle; no draft, no editor, no visible tests in the browser.],
  [Free file tree], [Closest to a real project.], [Publication cannot check that the tests will find what they need; every name comes from the student.],
  [List declared by the exercise, with optional additions], [The default stays one file; publication checks the list; each exercise chooses how much freedom it gives.], [Tabs, file names and a new contract to build and test.],
)

=== Decision

*Nothing changes by default.* An exercise that declares no files has one editable file, the `source` of the language pack (#adr("0013")). Existing exercises have nothing to write.

*The exercise declares its files per language pack*, since a standard I/O exercise is valid for several languages and `Main.java` means nothing in Python:

```json
"files": {
  "java": {
    "declared": [
      { "path": "Main.java" },
      { "path": "shapes/Shape.java", "readonly": true }
    ],
    "extra": 10
  }
}
```

The initial content of a declared file comes from `public/<pack>/<path>`; a declared file with nothing there starts empty. A read-only file must exist there. For the standard I/O runner, the list must hold the pack's `source`, which stays the entry point.

*One rule for paths.* A path is relative, made of segments matching `[A-Za-z0-9_][A-Za-z0-9_.-]*`, with a bounded depth and length; a leading dot, and so `..`, cannot match. One parsed type holds this rule, as `Id` does for identifiers. It applies at publication (#adr("0023")), in the API and again in the judge, which does not trust the queue.

*Student additions.* `extra` is the number of files a student may add, 0 when absent. The pack gains an `extensions` field, which limits what they may be called. An addition cannot take the path of a declared file, and two paths of a set cannot differ only by case.

*Read-only files.* Their tab cannot be edited, and they enter neither the draft nor the submission. The API refuses a new path that is read-only, and the judge writes the published copy, from the same release as the assessment data, whatever the browser sent. The browser receives them through the access gate (#adr("0020")) for its own runs. Private files stay in `assessment/`.

*One contract.* `files: [{ path, source }]` replaces `source` in submissions, drafts and jobs. There is no second field for the single-file case.

*Limits.* The current limit of 1 MiB applies to the total of an item. The number of files is bounded by the exercise schema (#adr("0023")). A submission holds at least one non-empty file.

*Drafts.* There is still one revision per item and one row per student and item (#adr("0038")). Adding, renaming or deleting a file is a write like any other. A `409 Conflict` shows both versions of the set, file by file. The encrypted buffer keeps the full set. A draft holds the set of one pack: changing language on a multi-language exercise asks for confirmation, then starts from that pack's templates. The final submission of #adr("0040") is this set.

*A new release during an activity loses nothing.* A write may keep a path that the current draft already holds, even if the release no longer allows it. Such a file counts as an addition while `extra` and `extensions` allow it, in path order; otherwise it is not judged, and the interface says so. A file that became read-only yields to the published copy.

*Judge and packs.* Every file is written to the directory mounted read-only, with one compilation per job (#adr("0033")). A compiled language's command names its inputs by a pattern that the judge expands in sorted order, without a shell: `**/*.c`, never the headers. Java and Python commands do not change: `java Main.java` finds the other source files since Java 22, and Python imports from the entry point's directory.

*Editor.* One tab and one #ext("codemirror")[CodeMirror] state per file, so undo history stays per file (#adr("0034")). The tabs can be reached by keyboard and are announced to screen readers, and Escape then Tab still leaves the editor. The control for adding a file appears only when `extra` is above 0; it reports an invalid name and asks for confirmation before a deletion. A diagnostic names its `path:line` and opens the right tab.

*Browser.* The worker writes the whole set to its virtual file system (#adr("0008")). Completion reads every file of the item (#adr("0035")).

*Out of scope.* Files of another kind created by the student, sub-projects and build tools such as Maven or Gradle, and archive upload. The manual "file upload" item of #adr("0015") is unrelated: nothing runs it.

=== Consequences

- The contracts change, and `drafts` and `jobs` need a migration, before any real data exists.
- `drafts` gains the language pack. The `final` job of #adr("0040") needs it anyway, since the table does not record it today.
- Each save sends the whole set, which may weigh on the write budget of #adr("0038"). This is measured before being optimized.
- A student's file can shadow a runner's module. A single file could already tamper with a runner in its own process, but `conftest.py` makes it easier: a runner keeps its own files off the student's import path, and its conformance suite gains these cases.
- The programs $D_i$ of #adr("0032") may have several files, with no other change.
- The assistant of #adr("0031") receives the whole set as the student's current code, within the same limits.

#validation(id: "V-0041")[
  An exercise without `files` behaves exactly as before. A Java exercise and a Python exercise of three files, one of them in a subdirectory (a `package` in Java), run under #ext("gvisor")[gVisor]. The publisher, the API and the judge each refuse `../x`, an absolute path and `.hidden`; the API and the judge also refuse a case-only duplicate, an extension outside the pack and one file more than `extra`. A read-only file sent straight to the API is refused and cannot change a verdict. Hostile additions (`conftest.py`, `pytest.py`, `org/junit/Test.java`) change no verdict. A conflict on a set shows both versions. After a release that removes a declared file, the student's copy is still in the draft and the next write succeeds. Under SEB for Windows and for macOS: tabs by keyboard, adding and deleting a file, the draft coming back after a restart, and reading a diagnostic from another file with NVDA and VoiceOver. Measure save latency at the size limit.
]
