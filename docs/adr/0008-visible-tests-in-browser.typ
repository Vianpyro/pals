#import "../template.typ": adr, arch, ext, mermaid, validation

== ADR-0008 — Running visible tests in the browser <adr-0008>

*Status:* proposed; the gain is to be quantified by load testing. Refined by #adr("0040"). Extended by #adr("0041") and #adr("0042"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("webassembly")[WebAssembly], #arch("load-and-performance")[Load and performance] and #arch("exam-mode")[Exam mode]; #adr("0001"), #adr("0007").

=== Context

During an exam, the time a student spends waiting on the queue is lost from their exam time. Giving it back is not a goal, since #ext("moodle")[Moodle] and #ext("ena")[Enaquiz] most likely do not allow it. The project instead aims to reduce the waiting caused by the platform itself.

Each exercise is submitted automatically at the end of the exam. During the exam, the server therefore only receives *test runs*, started when the student clicks "precheck" or "check" (#adr("0040")). The final submission does not wait for an immediate answer: its grading and its measurement (#adr("0007")) are deferred.

Each exercise has about ten visible tests and some hidden tests. For each visible test, the student sees the expected output and the actual output. For hidden tests, they only see whether at least one fails.

The server is located at ÉTS, on the same network as the exam rooms.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Run everything on the server],
  [A single execution path, faithful to the judge.],
  [Every click occupies the queue, even for a failing visible test.],

  [Visible tests in the browser for all languages],
  [Minimal server load.],
  [Unrealistic for Java, Rust or Go.],

  [Visible tests in the browser for languages where it is easy],
  [Every offloaded language shortens the queue for everyone; visible tests are public, so nothing leaks.],
  [Two execution paths; risk of discrepancy with the judge.],
)

=== Decision

Visible tests run in the browser for *every supported language for which it is easy*. A language is deemed easy if it meets three criteria:

- a maintained runtime, preloaded in one go and weighing at most a few tens of MB;
- a version that can be aligned with the judge's;
- no restrictive license.

The other languages keep running on the server.

#table(
  columns: (2.6cm, 1fr, 3.2cm),
  stroke: 0.5pt,
  [*Language*], [*Browser runtime*], [*Verdict*],
  [Python], [#ext("pyodide")[Pyodide]], [Retained],
  [JavaScript], [Native (#ext("web-workers")[Web Worker])], [Retained],
  [TypeScript], [Native, after transpilation by #ext("esbuild")[esbuild-wasm]], [Retained],
  [Lua], [#ext("wasmoon")[wasmoon]], [Retained],
  [Ruby, PHP], [#ext("ruby-wasm")[ruby.wasm], #ext("php-wasm")[php-wasm]], [To check],
  [C, C++], [clang compiled to WASM (30 to 100 MB)], [To evaluate],
  [C\#], [Roslyn and .NET in WASM], [Set aside],
  [Java], [#ext("cheerpj")[CheerpJ] (license) or javac and #ext("teavm")[TeaVM]], [Set aside],
  [Rust, Go], [No practical compiler], [Set aside],
)

Only the languages actually supported by a course are concerned.

Flow of a test run:

#mermaid(
  "
  sequenceDiagram
    participant B as Browser
    participant W as Web Worker
    participant A as API
    participant J as Judge
    alt retained language
      B->>W: run the visible tests
      W-->>B: results, displayed as they come in
      opt precheck, all visible tests pass
        B->>A: draft, no job
      end
      opt check, all visible tests pass
        B->>A: code
      end
    else other languages
      B->>A: code
    end
    opt code sent
      A->>J: job through the queue
      J->>J: one compilation, visible tests, hidden tests on a check
      J-->>A: NOTIFY per visible case as it ends, then with the verdict
      A-->>B: results over SSE, browser/judge discrepancy reported
    end
  ",
  document-context: true,
  width: 100%,
)

On a check, the code is sent to the server *only if all visible tests pass*, and a precheck that passes only saves the draft. Most attempts fail on a visible test and therefore never reach the server. On a check, the server runs the visible tests as well: the main cost is sandbox startup and compilation, so they add little, and they make it possible to detect a discrepancy between the browser and the judge, which is reported to the student.

Server results arrive through *#ext("sse")[Server-Sent Events]*, in a single stream per student, each visible case as soon as it ends (#adr("0030")). The judge notifies the API through #ext("postgresql")[PostgreSQL]'s #ext("listen-notify")[`LISTEN`/`NOTIFY`]. No new component is added.

On the server side, test runs share the same sandbox and a single compilation. Students are served in turn, with at most one run in progress per student. A new request replaces the previous one if it has not started yet. The hidden tests of a check stop at the first failure, and do not run if a visible test fails.

=== Consequences

- For each retained language, the judge's runtime has the same version as the browser's; for example, the Pyodide version pins the CPython version. The allowed packages are the same on both sides.
- The server remains the reference: the browser's result is presented as indicative.
- Runtimes are preloaded at the start of the exam by a #ext("service-workers")[service worker]. An infinite loop is interrupted by `terminate()` on the Web Worker.
- The SSE stream requires disabling buffering in #ext("nginx")[nginx]. After a disconnection, it resumes using `Last-Event-ID`; if the stream does not work, the client polls the server periodically.
- Compatibility with #ext("seb")[Safe Exam Browser] (WebAssembly, Web Workers, service workers) must be verified.

#validation(id: "V-0008")[
  Exam load test (about 250 students): compare the 95#super[th] percentile of the delay between clicking "check" and the server's answer, with and without in-browser execution. Measure, for each retained language, the rate of discrepancy between the browser and the judge. Verify that it works under Safe Exam Browser.
]
