#import "../template.typ": adr, arch, ext, validation

== ADR-0035 — Completion computed in the browser <adr-0035>

*Status:* proposed. Extends #adr("0013") and #adr("0034"). Extended by #adr("0041"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("safe-exam-browser")[Safe Exam Browser] and #arch("target-load")[Target load]; #adr("0008"), #adr("0031").

=== Context

An instructor asks for "local" completion based on the #ext("lsp")[Language Server Protocol]. "Local" is read here as computed on the student's machine, with no request to a server at each keystroke; this reading is to be confirmed (#arch("open-questions")[open question 25]). Students want it: 37 of 50 in practice and 36 of 48 in exams (#arch("appendix-student-survey")[student survey]).

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
  [Syntax tree and API index: a #ext("tree-sitter")[tree-sitter] grammar in WebAssembly parses the file; an index of the standard library, generated when the pack is built, lists each type's members], [Every language with a grammar; a few megabytes; members, signatures and documentation of the standard library.], [Knows only declared types: no inference through `var`, chained generics or lambdas; no diagnostics.],
  [Words only: the language's keywords and the identifiers already in the file], [Every language; nothing to download.], [No knowledge of types or libraries.],
)

=== Decision

*Completion runs in the browser only.* No language server runs on the platform's servers. A pack (#adr("0013")) declares its completion: `server` when a language server runs in a Web Worker, `index` when a syntax tree and an API index stand in for one, `words` otherwise, the default. Both `server` and `index` answer the editor's LSP client (#adr("0034")) from a worker, through the same message interface it would use over a network, so the editor does not know which one it talks to.

*The index tier.* The worker parses the student's file with the language's tree-sitter grammar, compiled to WebAssembly, to know the declared type of each variable, field and parameter in scope. After a `.` on a receiver of known type, it proposes that type's members from an index of the standard library (`java.util`, `java.lang` and `java.util.stream` for Java), with their signatures and the first sentence of their documentation. The index is generated from the language's own documentation when the pack's image is built, and pinned with it. Elsewhere it completes words, as `words` does. It reports no diagnostic: those still come from the judge. Its reach depends on how often a language declares types: in Java most variables carry one, whereas Rust's `let v = Vec::new();` and Go's `v := ...` leave the type to inference, so there the index mostly completes paths (`Vec::`, `strings.`) and annotated variables, and rarely a method after a `.`.

#table(
  columns: (3cm, 1fr, 3cm),
  stroke: 0.5pt,
  [*Language*], [*In the browser*], [*Status*],
  [JavaScript, TypeScript], [TypeScript's language service], [`server`, retained],
  [Python], [#ext("jedi")[Jedi] in #ext("pyodide")[Pyodide], already loaded for visible tests (#adr("0008")), or #ext("pyright")[Pyright], written in TypeScript], [`server`, to check],
  [C, C++], [#ext("clangd")[clangd] compiled to WebAssembly; size close to clang's (#adr("0008"))], [`server`, to evaluate],
  [C\#], [#ext("roslyn")[Roslyn] on .NET in WebAssembly, already used by browser playgrounds; tens of megabytes], [`server` to evaluate, `index` otherwise],
  [Java], [No language server runs in the browser: #ext("jdtls")[JDT LS] needs a JVM, and #ext("cheerpj")[CheerpJ] runs one only under licence, with tens of megabytes of libraries and a slow start], [`index`],
  [Kotlin], [Same as Java], [`index`],
  [Rust], [#ext("rust-analyzer")[rust-analyzer] compiles to WebAssembly but needs the standard library's sources; heavy for little demand. The index of `std` is generated from rustdoc's JSON output], [`index`],
  [Go], [No practical server: gopls calls the `go` command. The index of the standard library is generated from `go doc`], [`index`],
  [Others], [No practical server], [`words`],
)

*The worker is loaded with the editor's language*, served by the platform with immutable cache headers, never during the first keystrokes of an exam: it is fetched when the activity opens. Under SEB, the cache may not survive from one session to the next (to be checked), so an exam's opening is planned with every student downloading the worker at once: about 250 times its size within a few minutes (#arch("target-load")[Target load]). A `server` worker heavier than the measured budget falls back to `index` or `words` in exams.

*Completion is an activity setting.* The instructor chooses, per activity, `off`, `words` or `full`; `full` is the best tier the pack declares. The default is `full` in practice activities and `off` in exams. The setting is applied by the browser; it is not a security boundary, but under SEB the student cannot reach the developer tools to change it.

*Completion is deterministic.* It never calls the assistant's model (#adr("0031")). Diagnostics from a language server in the browser are shown as hints; the verdict and its diagnostics still come from the judge.

=== Consequences

- JavaScript and TypeScript get a language server first; Python, C/C++ and C\# depend on measurements; Java, Kotlin, Rust and Go get the index tier, weaker for Rust and Go where types are mostly inferred; the other languages get word completion.
- Java, the language most LOG200 students choose, never gets a language server: the index covers the standard library and the student's declared types, which is most of LOG200's code, but not inference or live diagnostics (#arch("appendix-student-survey")[student survey]).
- The index tier is one worker shared by every language, plus a grammar and an index per pack; it also answers students who find the Java documentation hard to use, since the signature and its documentation appear where they type.
- A language gains completion by adding a grammar and an index, or a server worker, to its pack, without touching the core.
- The judge and the API carry no completion load, during exams or otherwise.
- The download for a language server adds to the visible-test runtime of #adr("0008") for the same language.

#validation(id: "V-0035")[
  With the network cut after the activity opens, completion still works for a TypeScript and a Python item. No request leaves the browser while typing. An exam whose setting is `off` shows no completion under SEB. In a Java item, `new ArrayList<Integer>()` assigned to a declared `List<Integer>` proposes `add` with its signature after a `.`, with the network cut. Check whether SEB for Windows and for macOS keeps the worker in its cache between two sessions. Measure, per retained server and for the index tier: download size, memory in the browser, and time from keystroke to suggestions on an exam workstation; then the time for 250 browsers to fetch the worker at once from the platform.
]
