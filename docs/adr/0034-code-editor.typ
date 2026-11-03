#import "../template.typ": adr, arch, ext, plan, validation

== ADR-0034 — Code editor <adr-0034>

*Status:* proposed. Extends #adr("0013"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("safe-exam-browser")[Safe Exam Browser] and #arch("courses-and-roles")[Courses and roles]; #adr("0008"), #adr("0011"), #adr("0035").

=== Context

Every code item needs an editor, and the web application has none yet. An instructor asked whether to take #ext("monaco")[Monaco], the editor of VS Code, or to write one as #ext("ctester")[CTester] did.

CTester's editor is a `textarea` under a `pre` that paints the highlighting, with its own C highlighter, syntax check and shortcuts. It works because CTester has one language. The constraints here:

- The catalog has some thirty languages (#adr("0013")), each needing at least highlighting.
- It runs under #ext("seb")[Safe Exam Browser]: Chromium on Windows, WebKit on macOS; no CDN; shortcuts and the clipboard may be restricted (#arch("safe-exam-browser")[architecture]).
- The interface targets #ext("wcag")[WCAG 2.1 AA] with a keyboard-navigable editor, in English and French (#adr("0011")).
- Completion is requested (#adr("0035")), and compiler diagnostics should point at the line they are about.
- The platform is handed over to people who will maintain it (#plan("risks")[risk R10]).

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Custom, as in CTester], [Small; a native `textarea` is accessible and behaves predictably under SEB.], [A highlighter, a syntax check and shortcuts to write and maintain per language; no completion interface; the painted layer must stay aligned with the text (wrapping, input methods).],
  [#ext("monaco")[Monaco]], [The look of VS Code, which students know; TypeScript services built in.], [Its FAQ answers "No" to mobile browsers; large; speaks to a language server only through a third-party client.],
  [#ext("codemirror")[CodeMirror 6]], [Modular, each language loaded on demand; grammars for most of the catalog; an official #ext("lsp")[LSP] client under the MIT license (#ext("codemirror-lsp")[`@codemirror/lsp-client`], since 2025); touch screens supported; translatable interface strings.], [Looks less like VS Code; Pep/8 and some P3 languages need a small mode of their own.],
)

=== Decision

*CodeMirror 6, bundled and served by the platform.* One editor component serves every code field: code items, with their starting templates, and find-and-fix items (#adr("0032")). Nothing is loaded from a CDN.

*The language pack names its editor language.* A pack (#adr("0013")) gains one field, the CodeMirror language used for highlighting. A pack without one gets plain text: a missing grammar never blocks an item. The grammar is loaded when an editor for that language opens, not with the application.

*Run diagnostics are shown in the editor.* Compiler errors and the failing line of a visible test (#adr("0030")) are placed as markers on their line, through the editor's lint interface. The judge stays the authority on the verdict.

*CTester's invariants are kept, not its code.* Tab indents; Escape then Tab leaves the editor, so the keyboard is never trapped, and the editor says so to screen readers. Shortcuts and the clipboard are tested under SEB. Drafts are autosaved as before (#arch("mixed-assessments")[architecture]). The editor's own strings go through the interface translations (#adr("0011")). Collaborative editing, which CTester has, stays excluded (#plan("scope")[project plan]).

=== Consequences

- Highlighting for a new language is one field in its pack, plus a mode only for languages CodeMirror does not cover.
- Completion (#adr("0035")) plugs into the same editor through its LSP client, without changing the editor.
- The look differs from VS Code; shortcuts follow CodeMirror's defaults, which are close to it.
- The LSP client's repository moved off GitHub in April 2026; its source and version are pinned in the web application like any other dependency.

#validation(id: "V-0034")[
  Under SEB for Windows and for macOS: typing, Tab, Escape then Tab, undo, copy and paste work, and the draft comes back after SEB restarts. With a screen reader (NVDA, VoiceOver), a student can read a line, its diagnostic and the line number. Every P1 language is highlighted; a pack without an editor language opens as plain text. Measure the size loaded for the first code item and for each additional language.
]
