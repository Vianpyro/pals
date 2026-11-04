#import "../template.typ": adr, arch, course, ext, mermaid, validation

== ADR-0023 — Course content as untrusted input <adr-0023>

*Status:* proposed. Refines #adr("0005"), #adr("0008"), #adr("0012"), #adr("0013"), #adr("0015"), #adr("0016"), #adr("0021") and #adr("0022"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("zero-trust-principle")[Zero-trust principle] and #arch("hostile-content-author")[Hostile content author].

=== Context

The architecture treats student code as hostile and the teaching team as trusted. The platform is now meant to serve every LOG/GTI course and part of the DEG (#adr("0012")): dozens of instructors, lecturers and lab instructors, each able to push to a content repository, and each account a possible target. Zero trust applies to them too: one malicious or compromised content author must not be able to harm the rest of the school.

An author holds the following capabilities, all legitimate:

- pushing anything to the content repository of their course: statements, templates, images, tests, reference solutions, generators, item banks, judge configuration;
- the `instructor` or `ta` role in their offerings: preview, submission, schedules, re-judging, exam scheduling, CSV import (#adr("0012"), #adr("0020"));
- through their content, code that runs in the judge's sandbox and, for visible tests, in their students' browsers (#adr("0008")).

Several parts of the current design trust what these capabilities produce:

- Typst's #ext("typst-html-elem")[HTML export] accepts arbitrary elements, `<script>` included, and statements are delivered as inline HTML and SVG (#adr("0016")). Nothing filters the output. A statement could therefore run script in every student's session, in an instructor's preview or in the dashboard (#adr("0019")), or show a fake sign-in form that collects ÉTS directory passwords (#adr("0017")), which open far more than this platform.
- The publisher reads a Git tree whose symbolic links, submodules and identifiers are not constrained, and the course a release belongs to is not said to be fixed by the server. A link to `assessment/` or to a host file, or an identifier such as `../LOG200/x`, would leak or overwrite another course's data. The rendering cache is shared between courses.
- Some author code runs outside the sandbox: derived keys and calculated variants are computed at publication time (#adr("0015")), measurement inputs come from an author's generator (#adr("0007")), and non-code graders run in the judge process, where a formula evaluated with `eval` is code execution next to every course's assessment data.
- The judge configuration found in `assessment/` has no schema and no ceiling, and the sandbox mount of the private tests has no stated scope (#arch("open-questions")[open question 15]).
- The setup script of an #ext("oracle")[Oracle] exercise runs on a shared instance outside the sandbox (#adr("0013")).
- Visible tests written as unit tests are author code running in a same-origin Web Worker, which can call the API with the student's cookies (#adr("0008")).
- Deferred jobs, re-judging included, are charged to no one (#adr("0022")), and judges are reserved for any exam an instructor schedules (#adr("0021")).

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Trust the teaching team, with human review of content],
  [Nothing to build.],
  [Contradicts zero trust; a compromised account bypasses review; one reviewer cannot read every course's tests.],

  [Content in the database, edited through the application],
  [Every field typed by a form.],
  [Rejected by #adr("0005"): private data in the Internet-facing process; tests and solutions are still code.],

  [Content is untrusted input, confined to its course],
  [The damage an author can do stops at their offering; no review needed for safety.],
  [Restrictions on authors (no raw HTML, no custom image or command); a sanitizer and a second origin to operate.],
)

=== Decision

Course content is untrusted input. *The blast radius of a content author is their own offering.* What an author does to their own course (wrong grades, leaking their own exam, offensive content) is a matter of governance, traced but not prevented. Anything outside their offering must be impossible by construction, not merely forbidden.

+ *Identity is set by the server.* The course of a content repository comes from the platform's configuration, never from the content. Identifiers match `[a-z0-9-]+`. Release, pointer and cache paths are prefixed with the course on the server side, so no value from the content can name another course's file.
+ *Content is data.* The judge configuration follows a versioned schema (#adr("0014")): it picks a language pack and options from enumerations, and limits at or below the pack's ceilings. It never names an image, a command or a capability other than loopback networking (#adr("0013")). Formulas (calculated items, tolerances, symbolic answers) are parsed by a restricted grammar, never evaluated as code. Every piece of author code (reference solution, derived key, generator, checker, unit tests) runs as a deferred job in the judge's real sandbox, charged to the offering. Neither the publisher nor the judge process runs any.
+ *The publisher is confined to one course.* Each publication runs in its own container under #ext("gvisor")[gVisor], without network, secrets or database access. It sees only the tree exported from that repository, its course's release area and its course's rendering cache. The tree is read from Git objects: symbolic links, submodules, LFS pointers and non-regular files are refused, as are a file, a repository or an item count above their limits. A small trusted step then moves the `current` pointer, after checking that the release belongs to that course and matches its hash.
+ *Output is filtered and served apart.* HTML and SVG produced by Typst and merman go through an allow-list sanitizer: no `script`, no `on*` attribute, no `foreignObject`, no `form`, no `iframe`, no external reference. Every page carries a strict #ext("csp")[CSP] (`script-src 'self'`, `form-action 'self'`, `frame-ancestors 'none'`). Files in `public/` are served from a separate origin without cookies, with `Content-Disposition: attachment` and `X-Content-Type-Options: nosniff`. Since that origin sees no session, the API hands out a short-lived URL signed after the access gate, which nginx checks with #ext("nginx-secure-link")[`secure_link`]. Titles and other strings from the content are plain text everywhere, the dashboard included.
+ *Browser execution has no origin.* Visible tests (#adr("0008")) run in a Web Worker inside an #ext("iframe-sandbox")[iframe] with `sandbox="allow-scripts"`, whose origin is opaque, and `connect-src 'none'`. Neither the student's code nor the author's tests can reach the API.
+ *The sandbox sees one exercise.* The judge mounts only the `assessment/` subtree of the exercise being judged, read-only; never another exercise, and never another course. The runner's report is untrusted too: its schema and size are checked before a verdict is written.
+ *Judging services hold one schema.* An Oracle setup script runs as the user of the disposable schema, without any DBA privilege and with quotas, on an instance on its own VM and network segment. PostgreSQL stays ephemeral inside the sandbox.
+ *Every offering has a budget.* Deferred jobs started for an offering (re-judging, reference runs, derived keys) are charged to that offering's queue quota (#adr("0012")), not to no one (#adr("0022")); they still never count against a student's usage. Publications per hour are limited per course. The judges reserved for an exam are computed by the server from the group's enrollment (#adr("0021")); a reservation above a threshold needs an `admin`.
+ *Roles never come from the content's author.* An `admin` binds an LTI context to an offering. A CSV import only touches the importer's own offerings and only grants roles below the importer's. `admin` never comes from an enrollment source.
+ *Every publication is traced and can be stopped.* Each repository has its own read-only deploy key. A per-course webhook, signed with HMAC, only triggers a fetch and carries no data. The publication log records the push identity given by the Git host, since a commit's author field can be forged. An `admin` can freeze a course's publication, pin its pointer and suspend its quota.

A publication, confined to one course; the publisher container runs under gVisor, without network, secrets or database access:

#mermaid(
  "
  flowchart TD
    PUSH[Push to a course's<br/>content repository] --> WH[Signed per-course webhook<br/>no data, triggers a fetch]
    PUSH -.-> LOG[Publication log<br/>push identity from the Git host]
    WH --> F[Fetch with the repository's<br/>read-only deploy key]
    F --> X
    subgraph PUBC[Publisher container of this course]
      X[Tree exported from Git objects<br/>links, submodules, LFS refused] --> RND[Typst and merman]
      RND --> SAN[Allow-list sanitizer]
    end
    CACHE[Rendering cache<br/>of this course only] <--> RND
    SAN --> REL[Release area<br/>of this course only]
    REL --> TR{Trusted step: release of<br/>this course, hash matches?}
    TR -->|yes| PTR[current pointer moved]
    TR -->|no| KEEP[Pointer unchanged]
  ",
  document-context: true,
)

=== Residual risks

- An author can sabotage their own course. Audit and governance answer this, not the platform.
- A template handed to students can carry a `.vscode/tasks.json`, a `Makefile` or a script that runs on the student's own machine. This is outside the platform; course material at large carries the same risk.
- A crafted test can attempt to escape the sandbox, as student code can (#adr("0002")). Author code gains no position that student code does not already have.

=== Consequences

- Authors lose raw HTML and arbitrary images or commands. A need that the schema cannot express becomes a language pack or a question type in the platform repository, reviewed by its maintainer (#adr("0014")).
- A sanitizer is added to publishing, and nginx serves a second origin for public files.
- Publishing a course reads no other course and writes no other course's file; a bad or hostile push only affects that course's next release (#adr("0005")).
- The queue quota per offering now also bounds instructors, not only students.

#validation(id: "V-0023")[
  A hostile content repository is published as a test fixture. Each case must be refused or neutralized: a symbolic link to `/etc/passwd` and to `assessment/`; an `assessment/visible/` linked to another course's `hidden/`; the identifier `../LOG200/x`; `html.elem("script")` in a Typst statement and an SVG with `onload`; a sign-in `<form>` in a statement; a pytest file reading outside its exercise; the formula `__import__('os')`; a 10 GB test file; a publication of course A writing course B's pointer or cache; a visible test calling the API with `fetch`; an Oracle setup reading another schema; re-judging in a loop, which stays within the offering's quota.
]
