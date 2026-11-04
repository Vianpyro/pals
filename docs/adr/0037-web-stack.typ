#import "../template.typ": adr, arch, ext, plan, validation

== ADR-0037 — Web interface stack <adr-0037>

*Status:* proposed. Settles the library that #adr("0011") left to the web stack. \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("implementation-language")[Implementation language], #arch("cache")[Cache] and #arch("safe-exam-browser")[Safe Exam Browser]; #adr("0008"), #adr("0019"), #adr("0023"), #adr("0024"), #adr("0025"), #adr("0034"), #adr("0035").

=== Context

The web interface is one `index.html` and one `app.js`, plain JavaScript that builds the page by hand, with no build step. It covers sign-in, the catalog, a statement, a submission and the past runs. Several decisions already assume more:

- Static files are served by nginx, versioned by hash, and the previous release's files stay served (#arch("cache")[Cache], #adr("0024")). That needs a build that names files by their content.
- Nothing is loaded from a CDN, and the interface runs under #ext("seb")[Safe Exam Browser]: Chromium on Windows, WebKit on macOS (#arch("safe-exam-browser")[architecture]).
- The interface targets #ext("wcag")[WCAG 2.1 AA] and runs under a strict #ext("csp")[CSP] (#adr("0019"), #adr("0023")).
- Strings go through #ext("icu")[ICU MessageFormat] keys, and CI checks that `en` and `fr` have the same keys (#adr("0011")).
- #ext("codemirror")[CodeMirror 6] is bundled, and each language's grammar is loaded when an editor for it opens (#adr("0034")). The browser also loads large runtimes: #ext("pyodide")[Pyodide], #ext("esbuild")[esbuild-wasm] and TypeScript's language service (#adr("0008"), #adr("0035")).
- The platform is handed over to people who will maintain it (#plan("risks")[risk R10]).

The screens to come (question types, #adr("0015"); the dashboard, #adr("0019"); the run pipeline, #adr("0030")) carry far more state than the current page. Keeping that state in step with the page by hand does not scale.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Plain JavaScript, #ext("esbuild")[esbuild] to bundle], [The current code stays; one build dependency; nothing to learn.], [Every screen updates the page by hand; no accessibility checks beyond a linter; components and state handling invented by the project.],
  [#ext("svelte")[Svelte 5], #ext("vite")[Vite]], [Compiled, no virtual DOM at run time, small bundles; the compiler warns about accessibility mistakes (labels, keyboard handlers, ARIA roles); styles scoped per component; TypeScript supported.], [Fewer students know it than React; a Node toolchain for the build.],
  [Preact, Vite], [React's API, which more students know; about 4 KB.], [Accessibility checked only by a linter plugin; JSX and hooks rules to learn; a Node toolchain for the build.],
  [Server-rendered Rust (askama), htmx], [One language on the server; no client build.], [Moves rendering into the API, against a static interface served by nginx; the editor, completion and visible tests run in the browser anyway, so a client build is still needed.],
)

=== Decision

*Svelte 5 in #ext("typescript")[TypeScript], built by Vite into static files whose names carry their hash, served by nginx.* Node is only used to build, in CI and in the development container; the VMs receive the built files and run no JavaScript runtime, as #adr("0025") requires of the server.

*CI checks the interface.* `svelte-check` checks types and fails on the compiler's accessibility warnings. The key check of #adr("0011") runs on `locales/`.

*Translations use #ext("formatjs")[`intl-messageformat`].* It formats ICU messages; one `locales/<lang>.json` is loaded per language, and a missing key falls back to English as #adr("0011") requires.

*Heavy code is loaded on demand.* CodeMirror is mounted by a Svelte action. Its grammars, Pyodide, esbuild-wasm and the language services are separate chunks, loaded when an item needs them, never with the first page.

*Untrusted text stays text.* Svelte inserts text as text; `{@html}` is used only for HTML whose path #adr("0023") governs.

*Dependencies are pinned.* `package-lock.json` fixes every version and CI installs with `npm ci`, the CodeMirror LSP client included (#adr("0034")).

=== Consequences

- A Node toolchain enters the development container and CI, and a lockfile joins the dependencies to audit.
- `app.js` is rewritten as components, with the same behavior.
- Vite's hashed output gives the long-lived caching and the passive updates of #adr("0024") with no extra work.
- The successor learns Svelte, which fewer students know than React; its components are close to HTML.

#validation(id: "V-0037")[
  The production build loads under the strict CSP, without `'unsafe-inline'`. Sign-in, statement and submission work under SEB for Windows and for macOS. `svelte-check` reports no warning. A tab opened on the previous release keeps working after a deployment. Measure the size of the first page against the current page.
]
