#import "../template.typ": adr, mermaid

// Typst cannot list a directory: a new ADR must be added here.
// An ADR is never modified after acceptance; it is superseded by a new one
// that cites it ("supersedes ADR-000N").

= Architecture decision log <adr-log>

#table(
  columns: (2.2cm, 1fr, 3.5cm),
  stroke: 0.5pt,
  [*ADR*], [*Decision*], [*Status*],
  [#adr("0001", body: "0001")], [Submission queue in PostgreSQL], [Accepted, to validate],
  [#adr("0002", body: "0002")], [gVisor as initial isolation], [Accepted, to compare],
  [#adr("0003", body: "0003")], [NixOS for the VM], [Superseded (#adr("0006", body: "0006"))],
  [#adr("0004", body: "0004")], [Monorepo], [Accepted],
  [#adr("0005", body: "0005")], [Content publishing through immutable releases], [Accepted],
  [#adr("0006", body: "0006")], [Ubuntu and Ansible for the VM], [Accepted, to validate],
  [#adr("0007", body: "0007")], [Performance measurement by instruction counting], [Proposed],
  [#adr("0008", body: "0008")], [Visible tests run in the browser], [Proposed],
  [#adr("0009", body: "0009")], [Server-side verification of Safe Exam Browser], [Proposed, deferred],
  [#adr("0010", body: "0010")], [Multiple replicable, fault-tolerant VMs], [Proposed],
  [#adr("0011", body: "0011")], [User interface internationalization], [Proposed],
  [#adr("0012", body: "0012")], [Courses, offerings and per-course roles], [Proposed],
  [#adr("0013", body: "0013")], [Language packs and test runners], [Proposed],
  [#adr("0014", body: "0014")], [Stable core and extension points], [Proposed],
  [#adr("0015", body: "0015")], [Question types and mixed assessments], [Proposed],
  [#adr("0016", body: "0016")], [Mermaid diagrams in statements], [Proposed],
  [#adr("0017", body: "0017")], [Authentication against the ÉTS directory over LDAP], [Proposed],
  [#adr("0018", body: "0018")], [Build the judge or reuse an existing one], [Accepted, to validate],
  [#adr("0019", body: "0019")], [Operations and teaching dashboard], [Proposed],
  [#adr("0020", body: "0020")], [Schedules per course group], [Proposed],
  [#adr("0021", body: "0021")], [Bounded on-demand judges], [Proposed],
  [#adr("0022", body: "0022")], [Submission scheduling policy], [Proposed],
  [#adr("0023", body: "0023")], [Course content as untrusted input], [Proposed],
  [#adr("0024", body: "0024")], [Passive updates], [Proposed],
  [#adr("0025", body: "0025")], [Server implementation language], [Proposed],
  [#adr("0026", body: "0026")], [Spaced review activity mode], [Proposed],
  [#adr("0027", body: "0027")], [Self-assessed activities], [Proposed],
  [#adr("0028", body: "0028")], [LaTeX statements], [Proposed],
  [#adr("0029", body: "0029")], [Importing existing course material], [Proposed],
  [#adr("0030", body: "0030")], [Run progress shown as a pipeline], [Proposed],
  [#adr("0031", body: "0031")], [Comprehension assistant paid in points], [Proposed],
  [#adr("0032", body: "0032")], [Find-and-fix exercises and author checkers], [Proposed],
  [#adr("0033", body: "0033")], [Independent test cases and partial credit], [Proposed],
)

Relations declared in the status of each ADR: a solid arrow refines, a thick arrow supersedes, a dotted arrow applies, extends or feeds the ADR it points to. ADRs without a declared relation are not shown.

#mermaid(
  "
  flowchart LR
    A0001[0001 Queue in PostgreSQL]
    A0003[0003 NixOS]
    A0005[0005 Immutable releases]
    A0006[0006 Ubuntu and Ansible]
    A0008[0008 Visible tests in the browser]
    A0010[0010 Multiple VMs]
    A0012[0012 Offerings and roles]
    A0013[0013 Language packs]
    A0014[0014 Extension points]
    A0015[0015 Question types]
    A0016[0016 Mermaid in statements]
    A0018[0018 Build or reuse]
    A0020[0020 Group schedules]
    A0021[0021 On-demand judges]
    A0022[0022 Scheduling policy]
    A0023[0023 Untrusted content]
    A0024[0024 Passive updates]
    A0025[0025 Server language]
    A0026[0026 Spaced review]
    A0027[0027 Self-assessment]
    A0028[0028 LaTeX statements]
    A0029[0029 Content import]
    A0030[0030 Run pipeline view]
    A0031[0031 Comprehension assistant]
    A0032[0032 Find and fix]
    A0033[0033 Independent test cases]

    A0006 ==> A0003
    A0010 --> A0001
    A0010 --> A0006
    A0020 --> A0012
    A0021 --> A0010
    A0022 --> A0001
    A0012 -.->|applies| A0014
    A0013 -.->|applies| A0014
    A0015 -.->|applies| A0014
    A0016 -.->|extends| A0005
    A0018 -.->|applies| A0025
    A0021 -.->|judge reservation| A0012
    A0021 -.->|judge reservation| A0020
    A0022 -.->|queue priority| A0014
    A0025 -.->|applies| A0014
    A0026 -.->|applies| A0014
    A0027 -.->|applies| A0014
    A0027 -.->|extends| A0015
    A0027 -.->|self ratings| A0026
    A0023 --> A0005
    A0023 --> A0008
    A0023 --> A0012
    A0023 --> A0013
    A0023 --> A0015
    A0023 --> A0016
    A0023 --> A0021
    A0023 --> A0022
    A0024 --> A0010
    A0024 --> A0021
    A0028 -.->|extends| A0005
    A0028 -.->|applies| A0023
    A0029 -.->|feeds| A0015
    A0029 -.->|extends| A0005
    A0029 -.->|applies| A0023
    A0030 --> A0001
    A0030 -.->|extends| A0008
    A0030 -.->|applies| A0014
    A0031 -.->|applies| A0014
    A0031 -.->|applies| A0023
    A0032 -.->|extends| A0015
    A0032 -.->|applies| A0014
    A0032 -.->|applies| A0023
    A0033 --> A0008
    A0033 --> A0013
    A0033 -.->|extends| A0015
  ",
  document-context: true,
  width: 100%,
)

#include "0001-postgresql-queue.typ"
#include "0002-gvisor-isolation.typ"
#include "0003-nixos.typ"
#include "0004-monorepo.typ"
#include "0005-content-publishing.typ"
#include "0006-ubuntu-ansible.typ"
#include "0007-performance-measurement.typ"
#include "0008-visible-tests-in-browser.typ"
#include "0009-seb-verification.typ"
#include "0010-multi-vm-topology.typ"
#include "0011-ui-internationalization.typ"
#include "0012-multi-course-roles.typ"
#include "0013-language-packs.typ"
#include "0014-extension-points.typ"
#include "0015-question-types.typ"
#include "0016-mermaid-statements.typ"
#include "0017-ldap-authentication.typ"
#include "0018-build-or-reuse.typ"
#include "0019-dashboard.typ"
#include "0020-group-schedules.typ"
#include "0021-on-demand-judges.typ"
#include "0022-scheduling-policy.typ"
#include "0023-untrusted-content.typ"
#include "0024-passive-updates.typ"
#include "0025-server-language.typ"
#include "0026-spaced-review.typ"
#include "0027-self-assessment.typ"
#include "0028-latex-statements.typ"
#include "0029-content-import.typ"
#include "0030-run-pipeline-view.typ"
#include "0031-comprehension-assistant.typ"
#include "0032-find-and-fix-exercises.typ"
#include "0033-independent-test-cases.typ"
