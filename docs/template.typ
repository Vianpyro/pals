// Shared layout. Content documents only import the blocks
// (decision, hypothesis, validation, todo); only entry points apply
// `document` — otherwise an #include in the report would reapply the page
// setup and the title.
//
// Logo: curl -fsSL https://www.etsmtl.ca/assets/img/ets.svg -o docs/ets.svg
// (resolved relative to THIS file; `logo: none` to omit it).

#let _blue = rgb("#003087") // ÉTS navy blue
#let _red = rgb("#DA291C") // ÉTS red
#let _grey = luma(140)
#let _stripe = luma(248)

// Mermaid: merman draws through `layout`, which the HTML export ignores. In HTML
// the diagram therefore goes through html.frame (inline SVG). Import `mermaid`
// from here, never directly from merman.
#import "@preview/merman:0.3.0": mermaid as _mermaid
#let mermaid(..args) = context {
  // Inside html.frame, `width: 100%` has no reference width: the frame came
  // out as 0x0. Give it the width of an A4 page; the CSS rescales it.
  if target() == "html" { html.frame(block(width: 16cm, _mermaid(..args))) } else { _mermaid(..args) }
}

// ---------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------
//
// Every reference to another document or section is a link. Documents are
// compiled separately (one PDF and one HTML per entry point) and also included
// in the report, so a reference points inside the current document when it
// contains the label, and otherwise to the published document: its HTML page
// for the site, its PDF for a PDF.

#let site = "https://vianpyro.github.io/pals/"

#let xref(dest, slug, body) = context {
  if query(dest).len() > 0 {
    link(dest, body)
  } else if target() == "html" {
    link(site + slug + ".html#" + str(dest), body)
  } else {
    link(site + slug + ".pdf#nameddest=" + str(dest), body)
  }
}

// ADR headings carry <adr-NNNN>; architecture and project plan headings carry
// <arch-…> and <plan-…> (heading text in lowercase, words joined by "-").
#let adr(n, body: none) = xref(label("adr-" + n), "adr", if body == none { "ADR-" + n } else { body })
#let arch(name, body) = xref(label("arch-" + name), "architecture", body)
#let plan(name, body) = xref(label("plan-" + name), "project-plan", body)

#let course(code) = link("https://www.etsmtl.ca/etudes/cours/" + lower(code), code)

// External documents and tools, in one place so a moved URL is fixed once.
#let _ext = (
  access-act: "https://www.legisquebec.gouv.qc.ca/en/document/cs/A-2.1",
  acme: "https://www.rfc-editor.org/rfc/rfc8555",
  ags: "https://www.imsglobal.org/spec/lti-ags/v2p0",
  anki: "https://apps.ankiweb.net/",
  ansible: "https://docs.ansible.com/",
  archives-act: "https://www.legisquebec.gouv.qc.ca/en/document/cs/A-21.1",
  axum: "https://docs.rs/axum/",
  cargo: "https://doc.rust-lang.org/cargo/",
  certbot: "https://certbot.eff.org/",
  accros: "https://accros.etsmtl.ca/Rapports/index.asp",
  cheerpj: "https://cheerpj.com/",
  codemirror: "https://codemirror.net/",
  codemirror-lsp: "https://code.haverbeke.berlin/codemirror/lsp-client",
  codingame: "https://www.codingame.com/",
  coderunner: "https://coderunner.org.nz/",
  commonmark: "https://commonmark.org/",
  ctester: "https://github.com/Vianpyro/ctester",
  csp: "https://developer.mozilla.org/en-US/docs/Web/HTTP/Guides/CSP",
  deg-planning: "https://horaire.etsmtl.ca/Horairepublication/Planification-SEG.pdf",
  dmoj: "https://github.com/DMOJ/online-judge",
  docker: "https://docs.docker.com/",
  ena: "https://ena.etsmtl.ca/",
  entra: "https://learn.microsoft.com/en-us/entra/identity/",
  esbuild: "https://esbuild.github.io/",
  fastapi: "https://fastapi.tiangolo.com/",
  firecracker: "https://firecracker-microvm.github.io/",
  formatjs: "https://formatjs.github.io/docs/intl-messageformat/",
  fsrs: "https://github.com/open-spaced-repetition/fsrs-rs",
  gift: "https://docs.moodle.org/en/GIFT_format",
  github-actions: "https://docs.github.com/en/actions",
  gvisor: "https://gvisor.dev/",
  iframe-sandbox: "https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/iframe#sandbox",
  icu: "https://unicode-org.github.io/icu/userguide/format_parse/messages/",
  import-linter: "https://import-linter.readthedocs.io/",
  jedi: "https://jedi.readthedocs.io/",
  judge0: "https://judge0.com/",
  junit: "https://junit.org/",
  latex: "https://www.latex-project.org/",
  ldap: "https://www.rfc-editor.org/rfc/rfc4511",
  law25: "https://www.publicationsduquebec.gouv.qc.ca/fileadmin/Fichiers_client/lois_et_reglements/LoisAnnuelles/en/2021/2021C25A.PDF",
  leetcode: "https://leetcode.com/",
  listen-notify: "https://www.postgresql.org/docs/current/sql-notify.html",
  log200-survey: "https://forms.gle/ZWxyrpeUUaPUZuH88",
  lsp: "https://microsoft.github.io/language-server-protocol/",
  lti: "https://www.imsglobal.org/spec/lti/v1p3",
  mathml: "https://developer.mozilla.org/en-US/docs/Web/MathML",
  memcached: "https://memcached.org/",
  mermaid: "https://mermaid.js.org/",
  merman: "https://typst.app/universe/package/merman",
  monaco: "https://microsoft.github.io/monaco-editor/",
  moodle-questions: "https://docs.moodle.org/en/Question_types",
  moodle-xml: "https://docs.moodle.org/en/Moodle_XML_format",
  moodle: "https://moodle.org/",
  mpi: "https://www.mpi-forum.org/",
  nginx: "https://nginx.org/en/docs/",
  nginx-limit-req: "https://nginx.org/en/docs/http/ngx_http_limit_req_module.html",
  nginx-secure-link: "https://nginx.org/en/docs/http/ngx_http_secure_link_module.html",
  nginx-upstream: "https://nginx.org/en/docs/http/ngx_http_upstream_module.html",
  nixos: "https://nixos.org/",
  nrps: "https://www.imsglobal.org/spec/lti-nrps/v2p0",
  numpy: "https://numpy.org/",
  nvproxy: "https://gvisor.dev/docs/user_guide/gpu/",
  oidc: "https://openid.net/specs/openid-connect-core-1_0.html",
  open-mpi: "https://www.open-mpi.org/",
  openmp: "https://www.openmp.org/",
  otel-logs: "https://opentelemetry.io/docs/specs/otel/logs/data-model/",
  oracle: "https://www.oracle.com/database/free/",
  pandoc: "https://pandoc.org/MANUAL.html",
  parsons: "https://js-parsons.github.io/",
  pep8: "https://github.com/StanWarford/pep8",
  php-wasm: "https://github.com/seanmorris/php-wasm",
  podman: "https://podman.io/",
  postgresql: "https://www.postgresql.org/docs/current/",
  powershell-linux: "https://learn.microsoft.com/en-us/powershell/scripting/install/installing-powershell-on-linux",
  pyodide: "https://github.com/pyodide/pyodide",
  pytest: "https://docs.pytest.org/",
  qemu-user: "https://www.qemu.org/docs/master/user/main.html",
  qemu-insn: "https://www.qemu.org/docs/master/devel/tcg-plugins.html",
  rabbitmq: "https://www.rabbitmq.com/",
  re2: "https://github.com/google/re2",
  redis: "https://redis.io/",
  ruby-wasm: "https://github.com/ruby/ruby.wasm",
  rust: "https://www.rust-lang.org/",
  seb: "https://safeexambrowser.org/",
  seb-config-key: "https://safeexambrowser.org/developer/seb-config-key.html",
  service-workers: "https://developer.mozilla.org/en-US/docs/Web/API/Service_Worker_API",
  skip-locked: "https://www.postgresql.org/docs/current/sql-select.html#SQL-FOR-UPDATE-SHARE",
  slurm-fairshare: "https://slurm.schedmd.com/priority_multifactor.html",
  sqlx: "https://github.com/launchbadge/sqlx",
  sse: "https://html.spec.whatwg.org/multipage/server-sent-events.html",
  stack: "https://stack-assessment.org/",
  streaming-replication: "https://www.postgresql.org/docs/current/warm-standby.html",
  stress-ng: "https://github.com/ColinIanKing/stress-ng",
  svelte: "https://svelte.dev/docs/svelte/overview",
  systrap: "https://gvisor.dev/docs/architecture_guide/platforms/",
  teavm: "https://teavm.org/",
  terraform: "https://developer.hashicorp.com/terraform",
  texlive: "https://tug.org/texlive/",
  tokio: "https://tokio.rs/",
  tsan: "https://clang.llvm.org/docs/ThreadSanitizer.html",
  typescript: "https://www.typescriptlang.org/docs/",
  typst: "https://typst.app/docs/",
  typst-html-elem: "https://typst.app/docs/reference/html/elem/",
  typst-html: "https://github.com/typst/typst/issues/5512",
  ubuntu: "https://ubuntu.com/about/release-cycle",
  uvicorn: "https://uvicorn.dev/",
  valgrind: "https://valgrind.org/docs/manual/cl-manual.html",
  vite: "https://vite.dev/guide/",
  wasmoon: "https://github.com/ceifa/wasmoon",
  wcag: "https://www.w3.org/TR/WCAG21/",
  web-workers: "https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API",
)
#let ext(key, body) = link(_ext.at(key), body)

// ---------------------------------------------------------------------------
// Design blocks
// ---------------------------------------------------------------------------

#let _callout(label, fill, body, class: "") = context {
  if target() == "html" {
    html.elem("div", attrs: (class: "callout " + class))[*#label* #body]
  } else {
    block(fill: fill, inset: 10pt, radius: 3pt, width: 100%)[*#label* #body]
  }
}

// The identifier is optional: `#decision(id: "D-03")[...]`. It is used for
// traceability (requirement -> decision -> experiment -> result).
#let _heading(name, id) = if id == none { name + "." } else if id.starts-with("ADR-") {
  [#name #adr(id.slice(4)).]
} else { name + " " + id + "." }

#let decision(id: none, body) = _callout(_heading("Current decision", id), luma(245), body, class: "decision")
#let hypothesis(id: none, body) = _callout(_heading("Hypothesis", id), luma(250), body, class: "hypothesis")
#let validation(id: none, body) = _callout(_heading("To validate", id), luma(250), body, class: "validation")

// Section still to be written: visible, so it cannot be handed in by mistake.
#let todo(body) = context {
  if target() == "html" {
    html.elem("div", attrs: (class: "callout todo"))[_To write:_ #body]
  } else {
    block(
      fill: rgb("#fff4e5"),
      stroke: (left: 2pt + rgb("#e8a33d")),
      inset: 8pt,
      width: 100%,
    )[_To write:_ #body]
  }
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

#let document(
  title: "",
  subtitle: none,
  course: "LOG791",
  course-name: "Special Project in Software Engineering",
  students: ("Vianney Veremme",),
  term: "Fall 2026",
  group: none,
  supervisors: none,
  date: datetime.today().display("[year]-[month]-[day]"),
  logo: "ets.svg",
  slug: "document", // file name on the site (slug.html, slug.pdf)
  department: "Department of Software and IT Engineering",
  body,
) = {
  let running-header = course + " \u{2014} " + title

  set std.document(title: title, author: students)
  set text(font: "New Computer Modern", size: 10.5pt, lang: "en")
  set par(justify: true, leading: 0.65em, spacing: 1.2em)
  set heading(numbering: "1.1")
  show link: set text(fill: blue)

  show heading.where(level: 1): set text(size: 14pt, fill: _blue)
  show heading.where(level: 2): set text(size: 12pt, fill: _blue)
  show heading.where(level: 3): set text(fill: _blue)

  show raw.where(block: false): box.with(fill: luma(235), inset: (x: 3pt, y: 0pt), outset: (y: 3pt), radius: 2pt)
  show raw.where(block: true): block.with(fill: luma(240), inset: (x: 1em, y: 0.8em), radius: 4pt, width: 100%)

  let rows = (
    (if students.len() > 1 { "Students" } else { "Student" }, students.sorted().join(linebreak())),
    ("Course", course),
    ("Term", term),
  )
  if group != none { rows.push(("Group", group)) }
  if supervisors != none {
    rows.push((
      if supervisors.len() > 1 { "Supervisors" } else { "Supervisor" },
      supervisors.join(linebreak()),
    ))
  }
  rows.push(("Date", date))
  let info = table(
    columns: (auto, 1fr),
    stroke: 0.4pt + luma(200),
    inset: (x: 9pt, y: 6pt),
    fill: (_, row) => if calc.odd(row) { _stripe } else { white },
    ..rows.map(((k, v)) => (strong(k), v)).flatten(),
  )

  // ---- HTML (site): no page, no spacing; styling comes from site/style.css ----
  context if target() == "html" {
    html.elem("link", attrs: (rel: "stylesheet", href: "style.css"))
    html.elem("nav", html.elem("a", attrs: (href: "index.html"))[← Documentation])
    html.elem("header", attrs: (class: "title"))[
      #heading(level: 1, numbering: none, outlined: false, title)
      #if subtitle != none { html.elem("p", attrs: (class: "subtitle"), subtitle) }
      #html.elem("p", attrs: (class: "pdf"), html.elem("a", attrs: (href: slug + ".pdf"))[PDF version])
      #info
    ]
    outline(depth: 2)
    body
  } else {
    set page(
      paper: "a4",
      margin: (top: 2.5cm, bottom: 2.5cm, x: 2.4cm),
      header: context if counter(page).get().first() > 1 {
        set text(size: 9pt, fill: _grey)
        running-header
        v(-0.5em)
        line(length: 100%, stroke: 0.4pt + luma(210))
      },
    )

    // ---- Title page ----
    {
      set align(center)
      if logo != none {
        v(0.4cm)
        image(logo, height: 4.5cm)
        v(0.4cm)
      } else {
        v(1.6cm)
      }
      text(size: 14pt, weight: "bold", "École de technologie supérieure")
      linebreak()
      text(size: 10.5pt, fill: _grey, department)

      v(1.6em)
      line(length: 58%, stroke: 1.2pt + _red)
      v(1.6em)

      text(size: 22pt, weight: "bold", fill: _blue, title)
      if subtitle != none {
        v(0.4em)
        text(size: 14pt, fill: _blue, subtitle)
      }
      v(0.8em)
      text(size: 12pt, style: "italic")[#course \u{2014} #course-name]

      v(1.6em)
      line(length: 58%, stroke: 0.5pt + luma(190))
      v(1.6em)

      set align(left)
      info
    }

    pagebreak()
    set page(numbering: "1")
    counter(page).update(1)
    outline(depth: 2, indent: auto)
    pagebreak()
    body
  }
}
