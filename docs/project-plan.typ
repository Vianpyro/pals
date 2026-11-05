// Project plan — LOG791 (special project, Fall 2026), carried out alone under the supervising professor.
// Content only; entry point: pdf/project-plan.typ
#import "template.typ": adr, arch, course, ext, todo, xref

= Context and problem statement <plan-context-and-problem-statement>

#ext("ctester")[CTester] is an automated assessment platform for C programs, developed for the #course("TCH009") course and used in production: students write, compile and submit code there, which is judged by tests run in a #ext("gvisor")[gVisor] sandbox. It was designed for a single course, a single language and a single machine.

The #course("LOG200") course wants a comparable platform, with constraints that CTester does not cover: several languages, supervised assessments where an outage is critical, integration with #ext("moodle")[Moodle] and #ext("seb")[Safe Exam Browser], and hosting on ÉTS infrastructure. The MVP targets LOG200 (about 50 students), then #course("LOG121"); eventually, the platform must serve every LOG/GTI course and the computing courses of the DEG, for students and instructors alike.

*Problem.* How can we design a platform able to run untrusted code in several languages, withstand the load of a supervised exam and remain reusable from one course to another, on infrastructure with limited resources?

#todo[Have the problem statement validated by the supervising professor.]

= Objectives <plan-objectives>

+ Design and justify an architecture that separates the pedagogical application from the judge engine.
+ Implement a multi-language judge engine whose isolation is independent of the language.
+ Keep the core closed to special cases: LOG121 is added after LOG200 without modifying the core, only extension point implementations and content (#adr("0014")).
+ Demonstrate that the platform withstands a simulated exam load of 250 students, with latency thresholds defined in advance.
+ Make the infrastructure rebuildable from the repository.
+ Hand the platform over to the supervising professor and the ÉTS IT service, who will keep improving it.

#todo[Attach a measurable success criterion to each objective (P95 latency, tolerated failure rate), with a load profile taken from CTester's production logs (TCH009) rather than assumed.]

= Supervision and method <plan-supervision-and-method>

Individual project supervised by the supervising professor. Work proceeds in short iterations: each iteration implements what is needed to validate an architectural hypothesis, then records the decision in an ADR. Regular follow-up with the professor serves as a checkpoint.

#todo[Set the frequency of follow-up meetings.]

= Deliverables <plan-deliverables>

#table(
  columns: (4.5cm, 1fr),
  stroke: 0.5pt,
  [*Deliverable*], [*Content*],
  [Project plan], [This document.],
  [Architecture and ADRs], [High-level design and rationale for the choices, including the threat model.],
  [Prototype], [Deployable application, judge engine and isolation.],
  [Operations guide], [For the professor and the IT service: deploy, add a VM, add a language, restore a backup, prepare an exam.],
  [Final report], [Approach, measurement results and recommendations for LOG200.],
)

#todo[Confirm the required deliverables and deadlines with the supervising professor.]

#todo[Write the threat model (STRIDE): sandbox boundary, separation between API and judge, LDAP password transiting through the API.]

= Scope <plan-scope>

*Required.* Isolated execution of untrusted code; several languages; submission and structured verdict; confidentiality of tests; exam mode with reserved capacity; exams mixing every question type, from multiple choice to judged code, with Q1 types first (#adr("0015")); statements in Markdown or Typst with Mermaid diagrams (#adr("0016")); institutional authentication; roles per course offering (#adr("0012")); user interface in English and French (#adr("0011")). The model is designed for every LOG/GTI and DEG computing course; the MVP delivers LOG200, then LOG121.

*Constraints.* Hosting on ÉTS infrastructure, reachable only through the school's VPN; several VMs replicable by #ext("ansible")[Ansible] and fault-tolerant (#adr("0010")); 50 students for the MVP, then the scale of a department; personal data subject to Quebec's #ext("law25")[Law 25]; workload of an individual project course.

*Exclusions.* Social features, gamification, collaborative editing, plagiarism detection, content editing interface, mobile emulation, Windows commands and Windows Server roles, multi-service projects, translations into languages other than English and French (left to contributors). Moodle / Safe Exam Browser integration is optional, depending on the access obtained.

= Technical choices <plan-technical-choices>

The detailed analysis is recorded in the #arch("purpose-of-the-document")[architecture document] and the #xref(<adr-log>, "adr")[ADRs].

#table(
  columns: (3cm, 1fr, 3.2cm),
  stroke: 0.5pt,
  [*Question*], [*Solutions considered*], [*Chosen*],
  [Isolation], [container only, gVisor, #ext("firecracker")[Firecracker], WebAssembly], [gVisor (#adr("0002"))],
  [Queue], [#ext("postgresql")[PostgreSQL], #ext("redis")[Redis], #ext("rabbitmq")[RabbitMQ], file spool], [PostgreSQL (#adr("0001"))],
  [System], [#ext("nixos")[NixOS], distribution + Ansible, containers only], [Ubuntu + Ansible (#adr("0006"))],
  [Content], [database, served copy, immutable releases], [Releases (#adr("0005"))],
  [Authentication], [#ext("entra")[Entra ID] (OIDC), #ext("ldap")[LDAP] against Active Directory], [LDAP (#adr("0017"))],
  [Server language], [Python, Java, C++, #ext("rust")[Rust]], [Rust (#adr("0025"))],
  [Build or reuse], [extend CTester, #ext("judge0")[Judge0], #ext("dmoj")[DMOJ], #ext("coderunner")[CodeRunner]], [Port CTester's judge (#adr("0018"))],
)

Technologies: #ext("rust")[Rust] (#ext("axum")[axum], #ext("tokio")[tokio], #ext("sqlx")[sqlx]), #ext("postgresql")[PostgreSQL] (queue and #ext("streaming-replication")[streaming replication]), #ext("gvisor")[gVisor], #ext("docker")[Docker] or #ext("podman")[Podman], #ext("qemu-user")[QEMU user mode] (performance measurement), #ext("pyodide")[Pyodide]/WebAssembly (visible tests in the browser), #ext("ubuntu")[Ubuntu LTS] and #ext("ansible")[Ansible], #ext("nginx")[nginx] and #ext("certbot")[certbot], #ext("ldap")[LDAP] (ÉTS Active Directory), #ext("seb")[Safe Exam Browser], #ext("moodle")[Moodle] (#ext("lti")[LTI]), #ext("typst")[Typst], #ext("github-actions")[GitHub Actions].

= Schedule <plan-schedule>

#todo[Dated milestones: project plan, architecture, working prototype, load measurements, final report.]

= Risks <plan-risks>

#table(
  columns: (0.9cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*ID*], [*Risk*], [*Mitigation*],
  [R1], [The ÉTS VM is not available in time.], [Reproducible environment outside ÉTS; gVisor does not require KVM.],
  [R2], [Sandbox escape by hostile code.], [Defense in depth, no network or secrets in the judge, hostile tests in CI.],
  [R3], [Exam load not sustained.], [Early measurements; reserved capacity; backpressure.],
  [R4], [Scope too broad for one person.], [Explicit exclusions; prototype focused on judging.],
  [R5], [The platform does not work under SEB (a prerequisite for the project).], [SEB is free and installs without ÉTS: prototype tested from the first weeks with the platform added to the URL filter of a test `.seb` file, as the instructor does today for documentation sites; sign-in stays on the platform's domain (#adr("0017")); confirm early the SEB version and the exam workstation image. Config Key verification (#adr("0009")) comes after the MVP.],
  [R6], [Moodle access blocked.], [Optional Moodle integration: the exam starts directly in the platform.],
  [R7], [A VM fails during an exam.], [Stateless, redundant judges, recovery of abandoned jobs, PostgreSQL replica, VMs rebuilt by Ansible (#adr("0010")).],
  [R8], [A course requires a language or test format not planned.], [Language packs and test runners are extension points with a conformance suite: added without touching the core (#adr("0013"), #adr("0014")).],
  [R9], [The question-type catalog (some thirty types) is too large for one person.], [Types grouped into about nine grader families; delivered by tiers, Q1 with LOG200 (#adr("0015")).],
  [R10], [The professor and the IT service cannot maintain the platform after the handover.], [One server language (Rust, ranked above Python by the author and the successor, #adr("0025")); tools the IT service already operates; everything rebuildable by Ansible; operations guide delivered with the prototype; repository location agreed with the IT service.],
  [R11], [Student passwords leak through the platform, which receives them for the LDAP bind.], [LDAPS only; no password stored; logs carry no identity by construction (#arch("logs")[Logs]); rate-limited sign-in; covered by the threat model (#adr("0017")).],
  [R12], [The VPN keeps students from practising at home: 10 of 50 LOG200 students say having to turn it on would discourage them (#arch("appendix-student-survey")[student survey]).], [The sign-in page explains how to install and start the ÉTS VPN; practice from home is measured once the platform is in use; if it stays low, ask ÉTS to expose the student site outside the VPN.],
)
