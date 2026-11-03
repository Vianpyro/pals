#import "../template.typ": adr, arch, ext, validation

== ADR-0035 — Completion computed in the browser <adr-0035>

*Status:* proposed. Extends #adr("0013") and #adr("0034"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("safe-exam-browser")[Safe Exam Browser] and #arch("target-load")[Target load]; #adr("0008"), #adr("0031").

=== Context

An instructor asks for "local" completion based on the #ext("lsp")[Language Server Protocol]. "Local" is read here as computed on the student's machine, with no request to a server at each keystroke; this reading is to be confirmed (#arch("open-questions")[open question 25]).

The constraints:

- A language server reads the student's code, and some run it: rust-analyzer runs build scripts and procedural macros. On the server, each one would be untrusted code needing its own sandbox.
- Servers for compiled languages are heavy: clangd or the Java server hold hundreds of megabytes per open project. An exam brings about 250 students at once (#arch("target-load")[Target load]); none of that capacity is planned for editing.
- Completion is an aid. Whether it is allowed changes what an exam measures.
- Under #ext("seb")[Safe Exam Browser], everything is served by the platform.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [No completion], [Nothing to build.], [Does not meet the request.],
  [Language servers on the server, one sandbox per student, over a WebSocket], [Every language that has a server.], [Hundreds of megabytes per student; a new class of sandboxed jobs outside the queue; a round trip per keystroke; it competes with judging during exams.],
  [Language servers in the browser, in a #ext("web-workers")[Web Worker]], [No server cost; no latency; nothing leaves the machine.], [Only for languages whose server runs in JavaScript or WebAssembly; downloads to cache.],
  [Words only: the language's keywords and the identifiers already in the file], [Every language; nothing to download.], [No knowledge of types or libraries.],
)

=== Decision

*Completion runs in the browser only.* No language server runs on the platform's servers. A pack (#adr("0013")) declares its completion: `server` when a language server runs in a Web Worker, `words` otherwise, the default. The editor's LSP client (#adr("0034")) talks to the worker through the same message interface it would use over a network.

#table(
  columns: (3cm, 1fr, 3cm),
  stroke: 0.5pt,
  [*Language*], [*Server in the browser*], [*Status*],
  [JavaScript, TypeScript], [TypeScript's language service], [Retained],
  [Python], [#ext("jedi")[Jedi] in #ext("pyodide")[Pyodide], already loaded for visible tests (#adr("0008"))], [To check],
  [C, C++], [clangd compiled to WebAssembly; size close to clang's (#adr("0008"))], [To evaluate],
  [Java, Kotlin, C\#, Go, Rust, others], [No practical server], [Words only],
)

*The server is loaded with the editor's language*, served by the platform and cached, never during the first keystrokes of an exam: it is fetched when the activity opens.

*Completion is an activity setting.* The instructor chooses, per activity, `off`, `words` or `full`. The default is `full` in practice activities and `off` in exams. The setting is applied by the browser; it is not a security boundary, but under SEB the student cannot reach the developer tools to change it.

*Completion is deterministic.* It never calls the assistant's model (#adr("0031")). Diagnostics from a language server in the browser are shown as hints; the verdict and its diagnostics still come from the judge.

=== Consequences

- JavaScript and TypeScript get full completion first; Python and C/C++ depend on measurements; the other languages get word completion.
- A language gains full completion by adding a worker to its pack, without touching the core.
- The judge and the API carry no completion load, during exams or otherwise.
- The download for a language server adds to the visible-test runtime of #adr("0008") for the same language.

#validation(id: "V-0035")[
  With the network cut after the activity opens, completion still works for a TypeScript and a Python item. No request leaves the browser while typing. An exam whose setting is `off` shows no completion under SEB. Measure, per retained server: download size, memory in the browser, and time from keystroke to suggestions on an exam workstation.
]
