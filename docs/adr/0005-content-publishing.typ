#import "../template.typ": arch, ext

== ADR-0005 — Content publishing through immutable releases <adr-0005>

*Status:* accepted, carried over from #ext("ctester")[CTester] where it is in production. \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("content-lifecycle")[Content lifecycle].

=== Context

An exercise's content mixes public data (statement, templates) and private assessment data (tests, cases, answers). It is modified during the term, sometimes during a class session, and a publishing mistake can expose an answer key or remove an exercise from the menu the day before a class.

=== Options considered

- *Content in the database, edited in the application*: editing interface, but private data inside the student-facing process and a history to build.
- *Copy of the content repository served as is*: simple, but a single filtering mistake exposes the assessment data.
- *Public projection into immutable releases*: validation before writing, allow-list of published fields, content-addressed revision, pointer to the active release.

=== Decision

Content is published by projection into immutable releases. The active release is designated by a pointer, and the judge engine reads the assessment data directly from the content repository after re-checking that the exercise is open.

=== Consequences

- Rolling back content is a pointer rewrite, with no redeployment.
- The API can only expose what the projection wrote.
- #ext("typst")[Typst] statements are compiled at publication time; nothing is compiled per request.
- Editing goes through Git: the teaching team must be comfortable with this workflow, or an editing tool will have to produce it.
