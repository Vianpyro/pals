// Content only: the page and title are set by the entry point
// (pdf/architecture.typ) or by the report. Do not add #set page here.
#import "template.typ": adr, arch, course, decision, ext, hypothesis, mermaid, plan, validation

= Purpose of the document <arch-purpose-of-the-document>

This document presents the architecture envisioned for a programming learning and assessment platform, initially intended for the #course("LOG200") course at the École de technologie supérieure. The MVP targets about 50 students. #course("LOG121") comes second; eventually, the platform must serve every LOG/GTI course and the computing courses of the DEG, for students and instructors alike (see the #arch("appendix-course-inventory")[course inventory] at the end of this document).

The platform is a conceptual evolution of #ext("ctester")[CTester], a system originally developed for the automated assessment of C programs in the context of the #course("TCH009") course.

The goal is not simply to reimplement CTester in a new technology. Rather, the project seeks to identify the architectural properties that allow such a platform to be reused across several courses, to support several programming languages and to operate in supervised assessment settings.

This document is deliberately evolving. Not all the decisions presented here are final. When a decision still has to be validated by measurements or experiments, it is explicitly identified as such.

= Problem statement <arch-problem-statement>

The platform must allow students to write, compile and run code directly in a controlled environment.

This feature introduces a fundamental constraint: student code must be treated as *untrusted* code.

A submitted program may be incorrect by accident, but it may also behave in an extremely costly way or deliberately try to exploit the execution environment.

The platform must at the same time be able to support ordinary learning activities and supervised assessments that may involve several hundred students.

#decision[
  The architectural problem is therefore treated as one combining pedagogy, security, performance and operability rather than as a mere web interface problem.
]

The guiding question of the project is the following:

#quote[
  How can we design a programming learning and assessment platform able to run untrusted code in several languages, to support heavy loads during supervised assessments and to remain flexible enough to be reused in different teaching contexts?
]

= Objectives <arch-objectives>

== Functional objectives <arch-functional-objectives>

The platform must in particular allow:

- creating and publishing exercises;
- organizing exercises into activities, assignments and exams;
- every Moodle question type and the programming-specific ones, mixed freely in one exam, with pools and variants (#adr("0015"));
- writing code in a web environment;
- saving drafts;
- submitting programs;
- automated compilation and execution;
- test-based assessment;
- presenting a structured verdict;
- tracking progress;
- supporting several programming languages;
- integrating with #ext("moodle")[Moodle];
- use in a supervised exam context.

== Non-functional objectives <arch-non-functional-objectives>

The following properties are considered particularly important:

#table(
  columns: (2.8cm, 1fr),
  stroke: 0.5pt,
  [*Property*], [*Objective*],

  [Security], [Limit a student program's ability to access the hosting system, other submissions or sensitive data.],

  [Performance], [Keep latency acceptable even during heavy bursts of submissions.],

  [Scalability], [Allow judging capacity to grow independently of the main application's capacity.],

  [Reproducibility], [Be able to rebuild the infrastructure and deployment environments in an automated way.],

  [Extensibility],
  [Add a course, a language, a question type or an activity mode without modifying the core (#adr("0014")).],

  [Maintainability],
  [Keep responsibilities clearly separated, boundaries checked in CI and components testable independently (#adr("0014")).],

  [Transferability],
  [Be handed over to the supervising professor and the ÉTS IT service: one server language (Rust, #adr("0025")), tools the IT service already operates, a written operations guide.],
)

= Architectural principles <arch-architectural-principles>

== Separation of concerns <arch-separation-of-concerns>

The platform is designed around several distinct responsibilities.

#mermaid(
  "
  flowchart LR
      A[Student interface] --> B[API / Application]
      B --> C[Submission queue]
      C --> D[Judge engine]
      D --> E[Execution sandbox]
      E --> F[Compilation / Tests]
  ",
  document-context: true,
  width: 100%,
)

This separation notably allows the judge engine to evolve without modifying the pedagogical application.

== Zero-trust principle <arch-zero-trust-principle>

Student code must never be run directly by the application process.

#decision[
  The API never compiles or runs student code directly.
]

A submission is turned into an asynchronous job and handled by a component specifically responsible for running untrusted code.

This separation reduces the main application's attack surface and allows different resource policies to be applied to code execution.

Zero trust also covers the teaching team. Statements, templates, tests, reference solutions and judge configurations are written by many instructors, lecturers and lab instructors, any of whom may be malicious or have a compromised account.

#decision(id: "ADR-0023")[
  Course content is untrusted input. The blast radius of a content author is their own offering: what they do to their own course is traced, and anything outside it is impossible by construction. Content is data, author code only runs in the judge's sandbox, and the publisher, the sandbox mounts, the caches and the quotas are confined to one course (#arch("hostile-content-author")[Hostile content author]).
]

== Separation between application and judging <arch-separation-between-application-and-judging>

The main application is responsible in particular for:

* authentication;
* user management;
* courses;
* exercises;
* assignments;
* exams;
* results;
* progress;
* administration;
* Moodle integration.

The judge engine is responsible in particular for:

* receiving the jobs to run;
* scheduling their execution;
* selecting the appropriate runtime;
* enforcing resource limits;
* creating the isolated environment;
* compiling and running the program;
* running the tests;
* producing a verdict.

This separation also allows both parts of the system to be sized independently.

== Stable core and extension points <arch-stable-core-and-extension-points>

The courses to serve differ in language, question format and activity mode. The project is maintained by one person, so variation between courses must never accumulate as special cases in the core.

#decision(id: "ADR-0014")[
  The core knows no language, course or exercise by name. Everything that varies goes through an extension point made of declarative data, a versioned contract and a conformance suite run in CI. An extension point only exists if at least two real implementations are already known.
]

#table(
  columns: (3.4cm, 1fr),
  stroke: 0.5pt,
  [*Extension point*], [*Initial implementations*],
  [Question type], [Q1 types of each grader family (#adr("0015"))],
  [Activity mode], [Practice, assignment, exam],
  [Language pack], [P1 languages (#adr("0013"))],
  [Test runner], [Standard I/O, unit tests, SQL],
  [Judging service], [Ephemeral #ext("postgresql")[PostgreSQL], shared #ext("oracle")[Oracle]],
  [Enrollment source], [#ext("lti")[LTI 1.3], CSV import (#adr("0012"))],
  [Isolation backend], [#ext("gvisor")[gVisor], #ext("firecracker")[Firecracker]],
  [Statement renderer], [Markdown, #ext("typst")[Typst], with #ext("mermaid")[Mermaid] in both (#adr("0016"))],
)

An exam or an assignment is a list of items of any type combined with a mode. A course is an offering and a content repository, with no code of its own. Component boundaries (the API does not import the judge, the core imports no implementation) are checked by an architecture test in CI.

#validation[
  Adding LOG121 after LOG200 must only touch extension point implementations and content. Any core change it requires is recorded as an architecture defect.
]

= Overall architecture <arch-overall-architecture>

== Logical view <arch-logical-view>

The envisioned overall architecture is as follows:

#mermaid(
  "
  flowchart TB
    A[Student] --> B[Browser / SEB]

    subgraph AUTH[Authentication]
        IDP[ÉTS Active Directory]
    end

    RP[nginx reverse proxy<br/>TLS · static · rate limit]

    subgraph APP[Application]
        API[Web / API]
        DB[(PostgreSQL)]
    end

    subgraph EVAL[Assessment]
        Q[PostgreSQL queue<br/>Backpressure · Priorities · Fairness]
        S[Judge Scheduler]
        J1[Judge]
        J2[Judge]
        J3[Judge]
        JN[...]
    end

    subgraph SANDBOX[Isolation]
        SB[Sandbox Runtime]
        P[Student program]
        T[Private tests]
    end

    subgraph CONTENT[Content]
        CR[Content repository]
        PUB[Publishing<br/>validation · projection · Typst rendering]
        REL[(Public releases)]
    end

    CR --> PUB --> REL
    REL -->|read-only| API
    CR -->|assessment data| S

    B -->|HTTPS + session cookie| RP
    RP --> API
    API -->|LDAPS bind| IDP

    API --> DB
    API -->|Submission| Q

    Q --> S
    S --> J1
    S --> J2
    S --> J3
    S --> JN

    J1 --> SB
    J2 --> SB
    J3 --> SB
    JN --> SB

    SB -->|gVisor / Firecracker| P
    SB -->|Controlled access| T

    S -->|Verdict| DB
  ",
  document-context: true,
  width: 100%,
)

This diagram describes responsibilities rather than a final deployment topology.

Pedagogical content follows a path separate from submissions: it is published from its own repository, without going through the API or the queue.

#decision[
  The API only sees the public projection of the content. Only the judge engine reads the assessment data. The content lifecycle is detailed in its own section.
]

== Application and API <arch-application-and-api>

The web application is the part responsible for the pedagogical state.

It must not depend on the presence of a local compilation process. A submission is instead represented as a job that can be placed in a queue.

This makes it possible to decouple:

- the number of HTTP requests;
- the number of pending submissions;
- the number of available judges;
- the number of concurrent executions.

== Entry layer <arch-entry-layer>

The platform is deployed on several VMs with limited resources, replicable by #ext("ansible")[Ansible], so that a failure during an exam does not interrupt the service (#adr("0010")). Every additional infrastructure component consumes memory and CPU that are no longer available for judging.

#decision[
  An infrastructure component is only added when a measured need justifies it.
]

=== Reverse proxy <arch-reverse-proxy>

An #ext("nginx")[nginx] reverse proxy is the platform's only HTTP entry point. It is responsible for:

- TLS termination (#ext("acme")[ACME] certificates obtained and renewed by #ext("certbot")[certbot]);
- serving the web interface's static files;
- response compression;
- request rate limiting (#ext("nginx-limit-req")[`limit_req`]), notably on submissions;
- relaying the long-lived connections (SSE or WebSocket) used to notify verdicts.

#decision[
  nginx is chosen for its small memory footprint, its native rate limiting and its availability in the Ubuntu repositories.
]

The student site, like the dashboard (#adr("0019")), is reachable only through the ÉTS VPN, an ÉTS hosting constraint (#plan("scope")[project plan]). Practising from home therefore requires the VPN: in the #arch("appendix-student-survey")[student survey], 10 of 50 students say having to turn it on would discourage them, and 7 have never installed it.

#decision[
  Nothing is served without a valid session except the sign-in page (with a short presentation of the platform), the interface's static files and the ACME challenge. Every API route checks the session; the default is closed.
]

#hypothesis[
  If the institution already terminates TLS upstream of the VM, nginx remains useful for static files and rate limiting.
]

=== Load balancing <arch-load-balancing>

No dedicated load balancer is planned as long as the API fits on one VM.

- On the HTTP side, the API is one process per `web` VM whose asynchronous runtime (#ext("tokio")[tokio]) spreads connections over every core.
- On the judging side, judges _pull_ jobs from the queue rather than having them pushed. The queue therefore acts as the dispatcher itself, and capacity is adjusted by changing the number of judges.

Since several API instances may run at once (one per `web` VM, two during an update), none of them holds state in memory that another would need: quotas, presence and sessions live in PostgreSQL. CTester runs a single #ext("uvicorn")[uvicorn] worker precisely because its quotas and presence are held in memory. The same property lets a new API instance take over from the old one during an update, without interrupting the service (#adr("0024")).

#decision[
  If the API is spread over several `web` VMs, an nginx #ext("nginx-upstream")[`upstream`] block balances the load and removes a failed instance; this choice will be settled based on load tests (#adr("0010")).
]

== Submission queue <arch-submission-queue>

The queue is an important abstraction between the application and the judge engine.

It makes it possible to absorb load peaks without the application having to run the programs itself.

It will eventually need to support:

- exam priority;
- limiting the number of concurrent jobs;
- fair distribution of resources;
- backpressure;
- controlled retries;
- detection of abandoned jobs;
- measurement of waiting time.

#hypothesis[
  The exact queuing and scheduling policy will have to be determined
  experimentally based on the observed loads.
]

#decision(id: "ADR-0022")[
  Exam jobs are always claimed first. Other jobs are ordered by a base priority taken from their activity mode, plus a bonus for waiting time, minus a penalty for the judging time their author has used recently while others were waiting. The values are spaced so a new mode fits between two others. The policy is the claim query itself: no scheduler process is added.
]

#decision[
  The queue is initially implemented in PostgreSQL (#ext("skip-locked")[`SELECT … FOR UPDATE SKIP LOCKED`] and #ext("listen-notify")[`LISTEN/NOTIFY`]) rather than with a dedicated service such as #ext("redis")[Redis] or #ext("rabbitmq")[RabbitMQ].
]

This choice adds no service to the VM and places the queue state in the same transaction as the submission state. Retries, detection of abandoned jobs and exam priority can then be expressed directly in SQL.

The path of one submission, from the browser to the verdict:

#mermaid(
  "
  sequenceDiagram
    participant B as Browser
    participant N as nginx
    participant A as API
    participant D as PostgreSQL
    participant J as Judge
    participant S as Sandbox
    B->>N: submission (HTTPS + session cookie)
    N->>A: forwarded (limit_req)
    A->>D: job and submission in one transaction, NOTIFY
    D-->>J: NOTIFY wakes a waiting judge
    J->>D: claim: SELECT … FOR UPDATE SKIP LOCKED
    J->>J: resolve the exercise in the active release, recheck opening
    J->>S: compile and run with limits
    S-->>J: JSON report
    J->>D: write the verdict, NOTIFY
    D-->>A: LISTEN
    A-->>B: verdict over SSE
  ",
  document-context: true,
  width: 100%,
)

#validation[
  Load tests will have to confirm that PostgreSQL is sufficient as a queue for an exam load. A dedicated broker will only be considered if a limit is measured.
]

= Judge engine <arch-judge-engine>

== Multi-language abstraction <arch-multi-language-abstraction>

One of the major objectives is not to design the engine around C.

The envisioned conceptual model is:

#mermaid(
  "
  flowchart TD
    J[Judge]

    J --> LR[Language Runtime]
    J --> SB[Sandbox Runtime]
    J --> RL[Resource Limits]

    subgraph LANG[Supported Languages]
        C[C] -.-> C1[Compiler]
        PY[Python] -.-> PY1[Interpreter]
        JAVA[Java] -.-> JAVA1[Compiler / JVM]
        RUST[Rust] -.-> RUST1[Compiler]
        Other[...] -.-> Other1[...]
    end

    LR --> C
    LR --> PY
    LR --> JAVA
    LR --> RUST
    LR --> Other[...]
  ",
  document-context: true,
  width: 100%,
)

A language must mainly define how to:

- prepare the files;
- compile the program;
- launch the program;
- interpret its result;
- possibly manage its dependencies.

The isolation mechanism should not depend on the language.

#decision(id: "ADR-0013")[
  Each language is a declarative _language pack_: pinned image built in CI, compile and run commands, version and options (for C, the standard is an exercise option), default limits with a time multiplier, and capabilities (browser, instruction counting). Test formats are separate _runners_ (standard I/O, unit tests, SQL) that all produce the same JSON report.
]

== Language catalog <arch-language-catalog>

LOG200 aims for as many languages as possible, prioritized by industry use and its expected trend, with the #ext("leetcode")[LeetCode] and #ext("codingame")[CodinGame] catalogs as the horizon.

#table(
  columns: (1.4cm, 1fr, 3.2cm),
  stroke: 0.5pt,
  [*Tier*], [*Languages*], [*When*],
  [P1], [Java, Python 3, JavaScript, TypeScript, C, C++, C\#, Go, Rust, Kotlin], [With LOG200],
  [P2], [PHP, Ruby, Swift, Dart, Scala, Bash, SQL (PostgreSQL)], [After load validation],
  [P3],
  [Haskell, OCaml, Elixir, Erlang, Racket, Clojure, Lua, Perl, F\#, Groovy, VB.NET, Pascal, D, Objective-C, #ext("pep8")[Pep/8]],
  [On request or contribution],
)

Within P1, the LOG200 priority is Java, then Python, then JavaScript/TypeScript. Python is delivered first to build the end-to-end path, then Java (#adr("0013")).

Standard I/O is the default test format for LOG200: one set of tests is valid for every language. A function-signature harness requires a driver per language and is only added per exercise.

== Implementation language <arch-implementation-language>

The judge engine is likely to handle:

- many concurrent jobs;
- external processes;
- execution timeouts;
- resource limits;
- job queues;
- communication with the sandboxes;
- metrics collection.

It is also the only privileged process, and it receives hostile input: student code, course content (#adr("0023")) and possibly a compromised API.

#decision(id: "ADR-0025")[
  All server code, the judge engine included, is written in #ext("rust")[Rust], in one #ext("cargo")[Cargo] workspace that forbids `unsafe` code. Rust is the only language that both the author and the instructor who takes over the platform rank above Python; a single server language keeps it maintainable by them and by the ÉTS IT service.
]

Speed is not the reason: the dominant cost of a submission is expected in sandbox startup and compilation, not in the orchestrator (see #arch("cache")[Cache]).

The web interface runs in the browser, so it is written apart from the server code.

#decision(id: "ADR-0037")[
  The web interface is written in #ext("svelte")[Svelte 5] and #ext("typescript")[TypeScript], built by #ext("vite")[Vite] into static files that nginx serves. Node is only used to build them; the VMs run no JavaScript runtime.
]

= Submission isolation <arch-submission-isolation>

== Problem <arch-problem>

A classic container is not considered a sufficient security boundary on its own for running hostile student code.

The architecture must therefore distinguish:

1. the process management mechanism;
2. the container runtime, if any;
3. the main isolation mechanism;
4. resource limits.

Conceptually:

#mermaid(
  "
  flowchart LR
    J[Judge] --> S[Sandbox abstraction]

    subgraph CONSTRAINTS[Resource & execution constraints]
        FS[Filesystem]
        CPU[CPU]
        MEM[Memory]
        PROC[Processes]
        TIME[Timeout]
        NET[Network policy]
    end

    subgraph ISOLATION[Isolation boundary]
        G[gVisor]
        F[Firecracker]
        O[Other solutions]
    end

    S --> CONSTRAINTS
    S --> ISOLATION
  ",
  document-context: true,
  width: 100%,
)

== gVisor <arch-gvisor>

gVisor is currently the preferred candidate for the first implementation.

It provides an additional isolation layer between the executed program and the host Linux kernel.

#ext("systrap")[Systrap] mode is particularly attractive when the platform itself runs inside a virtual machine.

#decision[
  gVisor Systrap is the initially envisioned isolation solution.
]

#hypothesis[
  gVisor should offer a sufficiently good trade-off between security, performance and operational complexity for the LOG200 context.
]

== Firecracker <arch-firecracker>

Firecracker is a particularly attractive alternative when priority is given to a strong isolation boundary.

Its model relies on microVMs running their own Linux kernel under KVM.

On the other hand, the microVM lifecycle and the integration with the judge engine are more complex than an architecture based on containers.

#validation[
  gVisor and Firecracker should ideally be compared experimentally on the project's real workloads before the final choice is made.
]

== WebAssembly <arch-webassembly>

WebAssembly is another possibility thanks to its strongly sandboxed execution model.

However, using it for a general multi-language platform raises a different question: the student program must be runnable in a WebAssembly environment compatible with its language and libraries.

#decision[
  WebAssembly is considered a complementary avenue rather than the platform's
  initial universal environment.
]

This avenue is adopted on the client side: in languages where it is easy, visible tests run in the browser before anything is sent to the server (#adr("0008")). The judge remains the only reference.

== Container runtime <arch-container-runtime>

#ext("docker")[Docker] or #ext("podman")[Podman] can be used to manage the lifecycle of execution environments.

They are not, however, considered the main security boundary.

The target model is therefore rather:

#mermaid(
  "
  flowchart LR
    J[Judge] --> C[Container Runtime]
    C --> I[Isolation gVisor/Firecracker]
    I --> S[Student code]
  ",
  document-context: true,
  width: 100%,
)

#hypothesis[
  The exact choice between Docker and Podman should have less impact than the
  choice of the isolation mechanism itself.
]

= Resource management <arch-resource-management>

Each execution must have an explicit set of limits.

These include in particular:

#table(
  columns: (4cm, 1fr),
  stroke: 0.5pt,
  [*Resource*], [*Envisioned limit*],

  [Time], [Maximum timeout per step and per submission.],

  [CPU], [Number of cores or amount of usable CPU.],

  [Memory], [Memory limit per execution.],

  [Processes], [Maximum number of processes or threads.],

  [Storage], [Maximum temporary space.],

  [Network], [Network access explicitly denied or limited.],
)

These limits must be enforced regardless of the program's correctness.

A program stuck in an infinite loop must produce a timeout rather than consume a system resource indefinitely.

Under gVisor, the process limit also counts the threads of the sandbox's own kernel: in CTester, a sandbox with `--pids-limit` below 64 does not start, and fork bombs are stopped by the memory limit instead. Process limits are therefore set per language pack and checked by the conformance suite (#adr("0013")).

== Performance measurement <arch-performance-measurement>

Execution time is a safeguard, not a measurement. It varies with the VM's load and with the language, and therefore cannot be used to compare algorithms fairly.

#decision(id: "ADR-0007")[
  Performance is measured in executed instructions, deterministically, and compared with a reference solution in the same language, complexity being the main criterion. The limit of the measurement pass is an instruction budget. Measurement is done after the exam, on each student's final submission for each exercise.
]

= Load and performance <arch-load-and-performance>

== Target load <arch-target-load>

The MVP targets the final exam of one course: about 250 students (5 groups of 50). At the scale of the department, that is not the peak: a final exam is common to every group of a course, and several courses share the same final exam slot. The peak load is therefore the sum of the sessions that overlap in the busiest slot.

The final exam schedule changes every term; only its shape matters. On a draft schedule, counting 250 students per course in the busiest slot gives:

#table(
  columns: (1fr, 2.2cm, 2.6cm),
  stroke: 0.5pt,
  [*Courses counted*], [*Courses in the slot*], [*Students*],
  [Every course, without exception], [24], [6000],
  [Every course except final projects (no written exam)], [16], [4000],
  [Every LOG, GTI, MTI, MGL, INF and TCH course with an exam], [5], [1250],
  [The #arch("appendix-course-inventory")[course inventory] only], [3], [750],
)

#decision[
  The platform is sized for 1250 students in concurrent exams. 6000 is a theoretical ceiling, documented but not load-tested: it assumes that mechanical or electrical engineering courses hold programming exams on the platform. Graduate courses never have 250 students, so 1250 already includes a margin. The MVP load test targets 250 students.
]

It would, however, be incorrect to model the load as so many concurrent HTTP requests.

Each student may produce several submissions:

#mermaid(
  "
  flowchart LR
    A[Students of every session in the slot] --> B[Burst of submissions]

    B --> C[Judging queue]

    C --> D[Fix]
    D --> E[New submission]
    E --> C
  ",
  document-context: true,
  width: 100%,
)

The actual load therefore depends heavily on the students' behavior over time.

#hypothesis[
  At the end of an exam, each student submits about once every 3 minutes and a job takes about 2 s; drafts, presence and verdicts add about 0.4 database writes per second per student. By Little's law (concurrent jobs = arrival rate $times$ duration):

  #table(
    columns: (1fr, 1fr, 1fr, 1fr),
    stroke: 0.5pt,
    [*Students*], [*Writes/s*], [*Jobs/s*], [*Concurrent sandboxes*],
    [250], [≈ 100], [≈ 1.4], [≈ 3],
    [1250], [≈ 500], [≈ 7], [≈ 14],
    [6000], [≈ 2400], [≈ 33], [≈ 67],
  )

  PostgreSQL stays within its range even at the ceiling; the number of judge VMs is what varies. The load tests measure the two rates assumed here.
]

A first measure already refutes the duration for Java, and judges do not add up: a job takes longer when more of them share a machine (#adr("0039")). The table above is therefore a lower bound, kept to show how the load is reasoned about.

== Sizing <arch-sizing>

The system must allow the number of judge workers to be increased independently of the main application.

For example:

#mermaid(
  "
  flowchart TD
    API[Application / API] --> Q[Queue]

    Q --> J1[Judge worker]
    Q --> J2[Judge worker]
    Q --> J3[Judge worker]
    Q --> JN[...]

    J1 --> S1[Sandbox]
    J2 --> S2[Sandbox]
    J3 --> S3[Sandbox]
    JN --> SN[...]
  ",
  document-context: true,
  width: 100%,
)

This architecture allows capacity to be adjusted without modifying the pedagogical logic.

#decision(id: "ADR-0021")[
  Each judge VM keeps a floor of permanent judges and starts more, up to a measured cap, while jobs wait in the queue; an on-demand judge leaves after being idle and is never stopped from outside. The floor is raised before each scheduled exam session.
]

#decision(id: "ADR-0039")[
  A workflow computes a minimum and a recommended configuration per role from the measured load, and its latest report is the reference. A judge machine is sized from the rate it sustains at its ceiling, which is also the cap of #adr("0021"); the demand then gives the number of machines.
]

== Cache <arch-cache>

No dedicated cache server (Redis, #ext("memcached")[Memcached]) is planned initially. Caching is instead placed where it reduces a real cost:

#table(
  columns: (3.2cm, 1fr),
  stroke: 0.5pt,
  [*Location*], [*Content*],

  [Browser / nginx], [Static files versioned by hash and served with long-lived `Cache-Control` headers; `index.html` is not cached. The previous release's files stay served, so a tab opened before an update keeps working (#adr("0024")).],

  [API], [Published exercise data, rarely modified, kept in process memory.],

  [Judge],
  [Preloaded images and toolchains, sandboxes prepared in advance and, possibly, compiled artifacts of the private tests.],
)

Authentication is an #ext("ldap")[LDAP] bind against the ÉTS Active Directory (#adr("0017")). The session is a random identifier in a cookie, stored in PostgreSQL, which every API instance already shares: no dedicated session store.

#hypothesis[
  The dominant cost of a submission lies in sandbox startup and compilation rather than in data access. Judge-side caching should therefore have a greater impact than any application cache.
]

#decision[
  A shared cache will only be introduced if several API instances need to share the same state.
]

#hypothesis[
  A verdict cache, as in CTester, would absorb identical resubmissions during an exam. Its key covers the judge version, the language pack, the exercise's assessment data and the submitted sources. A timeout is never cached, since it depends on load rather than on code, and randomized exercises opt out. It comes after the MVP, once measurements show how often submissions repeat; every deployment invalidates it once.
]

== Metrics <arch-metrics>

Benchmarks will have to measure in particular:

- submission throughput;
- time spent in the queue;
- total latency;
- P50;
- P95;
- P99;
- compilation time;
- execution time;
- CPU usage;
- memory usage;
- maximum number of concurrent jobs;
- sandbox startup time per language, cold and pre-started;
- timeout rate;
- infrastructure failure rate;
- the share of test runs handled in the browser (#adr("0008"));
- the time to drain the measurement queue after an exam (#adr("0007")).

#decision[
  Performance will be assessed with reproducible workloads rather than with a theoretical estimate alone.
]

== Comparison of isolation mechanisms <arch-comparison-of-isolation-mechanisms>

An experiment may in particular compare:

#mermaid(
  "
  flowchart LR
    L[Identical load] --> G[gVisor]
    L --> F[Firecracker]

    G --> GM[Measurements]
    F --> FM[Measurements]

    subgraph METRICS[Metrics]
        M1[Startup time]
        M2[Latency]
        M3[Throughput]
        M4[CPU / memory]
    end

    GM --> METRICS
    FM --> METRICS

    METRICS --> C[Comparison]
  ",
  document-context: true,
  width: 100%,
)

The criteria will include at least:

- startup time;
- submission latency;
- throughput;
- memory consumed;
- CPU consumed;
- behavior under heavy load;
- behavior with malicious or pathological executions;
- operational complexity.

= Exam mode <arch-exam-mode>

The platform must distinguish the learning context from the assessment context.

Three conceptual modes are currently envisioned:

#mermaid(
  "
  flowchart LR
    P[Practice]
    A[Assignment]
    E[Exam]

    P --> P1[Exploration]
    P --> P2[Full feedback]

    A --> A1[Deadline]
    A --> A2[Progress]
    A --> A3[Submissions]

    E --> E1[Duration]
    E --> E2[Control]
    E --> E3[Reserved resources]
    E --> E4[Limited feedback]
  ",
  document-context: true,
  width: 100%,
)

#decision(id: "ADR-0040")[
  During an activity, a student runs a precheck (visible tests) or, if the activity allows it, a check (hidden tests too). The grade comes from the final submission, the last saved draft at the close, and appears only if the instructor allows it.
]

== Safe Exam Browser <arch-safe-exam-browser>

#ext("seb")[Safe Exam Browser] is considered a complementary mechanism rather than a feature the platform should reimplement.

SEB mainly controls the environment of the student's computer, while the platform controls the pedagogical state of the exam.

#mermaid(
  "
  flowchart LR
      subgraph PLATFORM[Platform]
        P[Pedagogical state of the exam]
        P --> P1[Exam]
        P --> P2[Exercises]
        P --> P3[Time]
        P --> P4[Submissions]
        P --> P5[Results]
    end

    subgraph SEB[Safe Exam Browser]
        S[Student environment]
        S --> S1[Lockdown]
        S --> S2[Allowed applications]
        S --> S3[Navigation]
        S --> S4[Configuration]
    end

    S -->|Controlled access| P
  ",
  document-context: true,
  width: 100%,
)

=== Constraints imposed by SEB <arch-constraints-imposed-by-seb>

- *Access.* For the MVP, the platform is added to the URL filter of the instructor's existing `.seb` file, like the documentation sites allowed today. Server-side verification of the SEB configuration is deferred as defense in depth (#adr("0009")).
- *Authentication.* The sign-in page is served by the platform (#adr("0017")): no external identity domain to allow and no phone-based multi-factor authentication, which the ban on phones would prevent.
- *Resources.* No CDN: browser runtimes (#ext("pyodide")[Pyodide], #ext("esbuild")[esbuild-wasm], etc.) are served by the platform.
- *Navigation.* No new windows and no downloads: statements are rendered as HTML or SVG, not PDF. Automatic submission redirects to SEB's "Quit URL".
- *Recovery.* If SEB is restarted, the student gets their autosaved drafts back, time is computed by the server and the SSE stream resumes using `Last-Event-ID`.
- *Editor.* Keyboard shortcuts and the clipboard, which SEB may restrict, are tested (#adr("0034")).
- *Engines.* SEB for Windows relies on Chromium; SEB for macOS and iOS rely on WebKit, where #ext("service-workers")[service workers] are limited. Fallback: the HTTP cache.
- *Language.* SEB does not pass on a language choice: the interface language is set in the platform itself (#adr("0011")).

== Reserved resources <arch-reserved-resources>

An exam must be able to rely on reserved judging capacity.

The goal is to prevent a non-critical activity from consuming all the workers while students are taking an exam.

Capacity and order are guaranteed separately, without separate infrastructure or capacity pools.

#decision[
  Before each scheduled exam session, each judge VM raises its floor of permanent judges to the reserved number (#adr("0021")). Exam jobs are protected in the claim query and are always claimed before any other job (#adr("0022")).
]

= Courses and roles <arch-courses-and-roles>

The platform serves several courses, each taught every term by several people in several groups.

#decision(id: "ADR-0012")[
  Data belongs to an _offering_ (a course in a term, e.g. LOG200 A2026), split into groups. Roles are held per offering, never globally: `student`, `ta` (results, no private tests), `instructor` (publishes, previews, runs exams for their groups), `coordinator` (every group of the course). Only `admin` is global, for platform operations.
]

- *Enrollments* come from Moodle through LTI 1.3 (the LTI context identifies the offering, #ext("nrps")[Names and Roles] provides the roster, #ext("ags")[Assignment and Grade Services] returns grades), or from a CSV import when Moodle is not available. The ÉTS Active Directory provides identity only (#adr("0017")).
- *Grades per activity*: a course splits its activities between Moodle and the platform, never mirroring all of them. An activity sends its grades to Moodle only if a Moodle LTI activity with a grade item points to it; the launch then carries its AGS `lineitem`. Any other activity keeps its results in the platform (results per group, CSV export). An activity that exists only in Moodle is unknown to the platform.
- *Schedules per group*: groups do not have their lectures, labs or contests at the same time. Opening and closing dates are entered per group by the instructor, in the platform, and never in the content (#adr("0020")).
- *Shared capacity*: exams are scheduled in advance and reserve judges for each group's time slot; outside exams, each offering has a queue quota.
- *Accommodations*: extra time and a shifted time slot, per student and per offering, relative to the group's window and applied by the activity mode; computed by the server. Only the measure is stored, never its reason.
- *Instructor tools* (minimum): dates per group, results per group, CSV export, re-judging an exercise after a test is fixed, individual extensions, preview.
- *Personal data*: an instructor only sees their offerings; retention is purged per completed offering (#ext("law25")[Law 25]).
- *Accessibility*: the interface targets #ext("wcag")[WCAG 2.1 AA], including a keyboard-navigable editor.

Enrollments and grades through Moodle:

#mermaid(
  "
  sequenceDiagram
    participant M as Moodle
    participant A as API
    participant D as PostgreSQL
    M->>A: LTI 1.3 launch (the context identifies the offering)
    A->>M: Names and Roles: roster
    M-->>A: members and their roles
    A->>D: recompute roles in this offering
    Note over A,D: without Moodle, a CSV import by the instructor
    Note over M,D: later, once results are final, for activities with a Moodle grade item
    A->>M: Assignment and Grade Services: grades
  ",
  document-context: true,
  width: 100%,
)

= Pedagogical model <arch-pedagogical-model>

The architecture must not limit an exercise to a "statement + solution" pair.

An exercise is instead considered a declarative resource containing pedagogical and assessment information.

Conceptually:

#mermaid(
  "
  classDiagram
    class Exercise {
      metadata
      statement
      language constraints
      difficulty
      skills
      prerequisites
      context
      tests
      hints
      release rules
    }
  ",
  document-context: true,
)

Public and private tests must remain separate.

#mermaid(
  "
  flowchart LR
    E[Exercise] --> P[Public data]
    E --> PR[Private data]

    P --> UI[Student UI]
    PR --> J[Judge]
  ",
  document-context: true,
  width: 100%,
)

This separation notably avoids exposing the test cases used to assess submissions.

== Question types <arch-question-types>

A code exercise is one kind of item among others. The platform must offer every #ext("moodle-questions")[Moodle question type] and those a programming course adds: Parsons problems, predicting a program's output, fixing a bug, clicking the faulty line.

#decision(id: "ADR-0015")[
  A question type is a declarative item schema bound to one of a few grader families. About nine graders cover some thirty types; a new type usually adds a schema and a display component, not a grader.
]

#table(
  columns: (2.8cm, 1fr),
  stroke: 0.5pt,
  [*Family*], [*Examples*],
  [Choice], [Single choice, multiple choice, true/false, dropdown, grid],
  [Key match], [Short answer, numerical with tolerance and units, regular expression, calculated],
  [Arrangement], [Matching, ordering, categorization, drag and drop into text, Parsons problem],
  [Composite], [Embedded answers (cloze)],
  [Judged code], [Full program, function or class, fill-in code, fix the bug, SQL, student-written tests],
  [Derived key], [Predict the output, choose the complexity (key computed from the reference)],
  [Area selection], [Hotspot on an image, markers on an image, faulty line of code],
  [Math expression], [Symbolic answer checked for equivalence],
  [Manual], [Essay, file upload, UML diagram, code review (rubric)],
)

The full catalog and the Q1–Q3 tiers are given in #adr("0015"). Non-code graders are pure functions over data: they run in the judge, which remains the only reader of assessment data, but without a sandbox. Regular expressions run on a linear-time engine (#ext("re2")[RE2]).

== Mixed assessments <arch-mixed-assessments>

An exam is an activity mode and an ordered list of slots. A slot holds a fixed item or draws $n$ items from a pool of the course bank; items of every family can be mixed in the same exam.

#decision[
  Draws, shuffles and calculated variants are derived from a seed (offering, exam, student) computed by the server. A reload or a Safe Exam Browser restart shows the same exam, and nothing about the variant is decided by the browser.
]

Each item is autosaved as a draft. The exam's score is the sum of item scores; a code item keeps its full verdict, and manual items wait in the grading queue of the offering's TAs (#adr("0012")).

== Extensibility <arch-extensibility>

Adding an exercise should ideally be mainly a configuration and content operation rather than a change to the platform's source code.

The architecture must therefore favor a declarative model.

The assessment engine interprets the exercise data and selects the runtime matching the requested language.

= Content lifecycle <arch-content-lifecycle>

In CTester, most content-related incidents did not come from the judge, but from the path between the instructors' repository and what the student receives: an answer key exposed by accident, an exercise disappearing from the menu the day before class, an empty catalog served silently. This path is therefore treated as an architectural component in its own right.

== Overview <arch-overview>

#mermaid(
  "
  flowchart LR
    D[Private content<br/>repository] --> V[Validation]
    V --> P[Public projection]
    P --> R[Immutable release<br/>revision = hash]
    R --> PTR[current<br/>pointer]

    PTR --> API[API<br/>public data]
    D --> J[Judge<br/>private tests]
    PTR -.->|re-checks opening| J
  ",
  document-context: true,
  width: 100%,
)

Content is edited in a repository separate from the platform's code. Each exercise groups its metadata, its statement, its public files (templates) and its private assessment data:

```text
exercises/<id>/
├── exercise.json      metadata, skills (no dates: ADR-0020)
├── statement.md       or statement.typ, never both
├── public/            templates handed to the student
└── assessment/        tests, cases, judge configuration (private)
    ├── visible/       cases shown with expected and actual output (ADR-0030)
    └── hidden/        cases never detailed, only whether one fails (ADR-0008)

bank/<id>.json         a non-code item: public part and private key
exams/<id>.json        mode, sections, slots (fixed item or pool draw), points
```

#decision[
  Publishing is triggered by a change to the content repository and requires no redeployment or restart of the application or the judge.
]

== Validation <arch-validation>

Invalid content must never replace the active publication.

Validation checks in particular the metadata schema, the uniqueness of identifiers, the consistency of collections and the absence of ambiguity, for example two statement formats for the same exercise. It fails *before* the first write. An exercise still carrying the report of a lossy import is refused (#adr("0029")).

#decision[
  A validation error stops publishing, naming the faulty exercise and field. The previous release keeps being served.
]

== Public projection <arch-public-projection>

The public release is not a copy of the content repository. It is rebuilt field by field from an explicit list of what may be shown.

A second check then re-reads the produced projection and refuses to publish if a key reserved for assessment data (`answer`, `expect`, `cases`, `stdin`, `correct`, `key`, `tolerance`, `pairs`, `order`, `regions`, internal paths…) appears in it.

#decision[
  The projection is built by positive enumeration (what is published) and checked by negative enumeration (what must never be). The first protects against today's oversight, the second against the field added tomorrow.
]

The published catalog contains *all* exercises. The API shows each student every exercise, including those not yet open, with the dates of their own group (#adr("0020")). An exercise's details (statement, templates, questions) are only served through the access gate.

#hypothesis[
  Showing a locked exercise with its date is better than hiding it: in CTester, an exercise missing from the menu was perceived by students as an outage.
]

== Releases and rollback <arch-releases-and-rollback>

Each publication produces a release directory whose identifier is the hash of its content. Republishing unchanged content creates nothing, and a change creates a new release that coexists with the previous ones.

The active release is designated by a pointer, a file rather than a symbolic link. A container mount resolves the link at startup, so a change of link would only be visible after a restart.

#decision[
  Rolling back content consists of rewriting the pointer to a previous release. It is instantaneous and redeploys no component. Each course has its own content repository and its own pointer: rolling back one course leaves the others unchanged (#adr("0012")).
]

A publication, with the cases where it stops:

#mermaid(
  "
  flowchart TD
    C[Change to a course's<br/>content repository] --> V{Valid?<br/>schema, unique identifiers,<br/>a single statement format}
    V -->|no| K[Publishing stops and names<br/>the exercise and field.<br/>The previous release keeps being served.]
    V -->|yes| P[Public projection<br/>positive enumeration]
    P --> N{Reserved key in<br/>the projection?<br/>negative enumeration}
    N -->|yes| K
    N -->|no| H{Release with this<br/>hash already exists?}
    H -->|yes| U[Nothing is created]
    H -->|no| R[New immutable release]
    R --> PTR[The course's current<br/>pointer is rewritten]
    R --> PR[Old releases pruned<br/>by manifest date]
    RB[Rollback] -->|pointer rewritten to<br/>an earlier release| PTR
  ",
  document-context: true,
)

CTester made the opposite choice when it gained a second course: every content repository is merged into one release, published all or nothing, so a bad commit in one course blocks the publication of every course. A pointer per course avoids exactly this.

Pruning keeps the latest releases according to a publication date written in their manifest, not according to the file system's modification time. In CTester, the latter had a different granularity on Windows and on Linux, which caused a release that was supposed to be kept to be deleted.

== Opening over time <arch-opening-over-time>

The content only carries a state (`draft`, `archived`). Dates belong to the offering's schedule: groups of the same course do not have their labs, assignments or contests at the same time, and one content repository serves every group and every term (#adr("0012")).

#decision(id: "ADR-0020")[
  A single function decides whether an exercise is accessible to a student at a given instant: the student's extension, otherwise their group's dates, otherwise the offering's; then the student's accommodation (extra time, shifted start) if the activity mode applies it. A session closes at its last student's window. Without dates for the student's group, the exercise is closed. An exercise opens when its date passes, without any commit or scheduled task on the morning of the class, and an instructor's postponement applies without publication.
]

#mermaid(
  "
  flowchart TD
    Q[Is this activity open<br/>for this student now?] --> O{Student<br/>extension?}
    O -->|yes| W[Use that window]
    O -->|no| G{Row for the<br/>student's group?}
    G -->|yes| W
    G -->|no| F{Offering row<br/>without a group?}
    F -->|yes| W
    F -->|no| C[Closed]
    W --> A{Accommodation, and<br/>the mode applies it?}
    A -->|yes| X[Extra time,<br/>shifted start]
    A -->|no| T
    X --> T{Now inside<br/>the window?}
    T -->|yes| OP[Open]
    T -->|no| C
  ",
  document-context: true,
)

The access gate, the catalog, the judge's double check and the preview all call this function.

All reads of an exercise (details, submission, draft, discussion) go through a single gate that resolves the identifier in the active release and refuses anything that is not open. A link shared ahead of time therefore bypasses nothing.

== Double check by the judge <arch-double-check-by-the-judge>

The API only passes an exercise identifier to the judge. The judge itself resolves this identifier against the active release before reading the assessment data.

#mermaid(
  "
  sequenceDiagram
    participant A as API
    participant D as PostgreSQL queue
    participant J as Judge
    participant R as Active release
    participant P as Assessment data
    A->>D: job with the exercise identifier only
    J->>D: claim the job
    J->>R: resolve the identifier
    J->>J: open for this student now? (schedule, role in the offering)
    alt closed or unknown
      J->>D: refused, no test runs
    else open
      J->>P: read the tests of this exercise
    end
  ",
  document-context: true,
  width: 100%,
)

#decision[
  The judge does not trust the API about whether an exercise is open. A compromised API can lie about who authored a submission, but cannot get the tests of a closed exercise executed.
]

== Instructor preview <arch-instructor-preview>

An instructor must be able to view and submit an exercise before it opens, under real conditions, without opening it to students.

The projection therefore writes a restricted copy of unopened exercises, served only to the instructors of that course and never cached by an intermediary. The role in the offering (#adr("0012")) is recomputed server-side on every request, and again by the judge, from the authenticated identity.

#decision[
  Preview is a property of the authenticated identity, not a global flag. The access gate's default behavior is closed.
]

== Content proof <arch-content-proof>

An incorrect test produces a wrong verdict that the student cannot contest.

#decision[
  Each exercise comes with a reference solution kept outside the published repository. A check compiles this solution and runs it through the real judge before opening. An exercise without a solution is flagged as "unproven".
]

#validation[
  This check must be integrated into the content repository's CI, and unproven exercises must be visible before their opening date.
]

= Statement rendering <arch-statement-rendering>

A programming statement contains text, code, formulas and sometimes tables, figures or diagrams. Two formats are supported, with opposite rendering models. They apply to every text field of every item (stem, choices, feedback, hints), not only to exercise statements, and both accept #ext("mermaid")[Mermaid] diagrams. Exercise statements may also be written in #ext("latex")[LaTeX] (#adr("0028")).

#table(
  columns: (2.4cm, 1fr, 1fr, 1fr),
  stroke: 0.5pt,
  [], [*Markdown*], [*Typst*], [*LaTeX*],
  [Use], [Default, the vast majority of statements], [Tables, diagrams, figures, multi-page layout], [Instructors who write LaTeX; exercise statements only],
  [Rendering], [In the browser, at display time], [At publication time, in a container], [At publication time, by pandoc in a container],
  [Delivered], [Source text], [HTML, with light and dark SVG as fallback], [HTML and MathML],
  [Accessibility], [Full], [Reduced for SVG], [Full],
  [Mermaid], [Fenced `mermaid` block, SVG at publication time], [`mermaid(...)` from the template], [`mermaid` environment, as a fenced block],
)

== Markdown <arch-markdown>

Markdown is rendered client-side by a deliberately restricted grammar: paragraphs, headings, lists, inline code, highlighted code blocks and emphasis. A general-purpose library is not used.

This decision stems from two observations made in CTester:

- a full Markdown library and its sanitizer weighed several times the rest of the page, on the path of students who are not logged in;
- #ext("commonmark")[CommonMark]'s rules are poorly suited to C: the asterisk is also the dereference and multiplication operator, and `*quotient and *remainder` becomes italics, losing both asterisks.

#decision[
  Emphasis is only recognized when the delimiter touches a word on the inner side and is separated from the text on the outer side. The underscore `_` is not an emphasis syntax, since it appears in most identifiers.
]

Formulas are explicitly delimited by `$…$` and converted to native #ext("mathml")[MathML]. The browser draws them without any library or external font, which requires no exception to the #ext("csp")[content security policy] (CSP).

#decision[
  A formula is never guessed. In C, `z/4` is an integer division and not a fraction, and a fraction bar would teach the opposite. A formula that does not parse is displayed as inline code, never as an error.
]

Markdown content comes from the course's content repository, which is untrusted (#adr("0023")). The grammar has no raw HTML and no links, and any HTML output is built from escaped fragments.

#hypothesis[
  Content written by students (forum, discussions) requires a different pipeline: escaping before parsing and allow-list sanitization on every display. The two pipelines must not be merged.
]

== Typst <arch-typst>

Typst is reserved for what the Markdown grammar cannot express.

#mermaid(
  "
  flowchart LR
    S[statement.typ] --> C[Copy without<br/>private data]
    C --> T[typst container<br/>--root · no network]
    L[Shared template<br/>vendored packages] --> T
    T --> H[HTML]
    T --> SV[Light / dark SVG]
    H --> R[Release]
    SV --> R
  ",
  document-context: true,
  width: 100%,
)

#decision[
  Nothing is compiled per request. Typst runs during publishing, and the student receives static files. The student-facing service has no Typst compiler, no access to private content and no access to the container runtime.
]

Compilation treats the document as untrusted, even if it is written by the teaching team:

- it is done from a *copy* of the exercise that does not contain the assessment data;
- the Typst project root is limited to this copy, which rejects outgoing relative paths and re-roots absolute paths;
- the copy is exported from Git objects, and symbolic links, submodules and non-regular files are refused;
- the container runs under #ext("gvisor")[gVisor], has no network access, and the packages used (course template, #ext("mermaid")[Mermaid]) are vendored;
- a maximum timeout and a memory limit bound a heavy document that would block publishing;
- the produced HTML and SVG go through an allow-list sanitizer: Typst's HTML export accepts any element, `script` included (#adr("0023")).

The course template is distributed as a local Typst package. The instructor writes no preamble: the platform applies the template, then includes the statement.

=== Themes and formats <arch-themes-and-formats>

An SVG is painted once and for all and cannot follow the page's theme. Each statement is therefore rendered twice, in light and in dark.

Typst's HTML export is preferred when it is complete. Publishing detects elements ignored by the HTML export and then publishes only the SVGs.

#decision[
  HTML is displayed first and SVG serves as a fallback, with no choice exposed to the student. An HTML failure does not block publishing; an SVG failure does.
]

#mermaid(
  "
  flowchart LR
    T[statement.typ] --> SVG{Light and dark<br/>SVG rendered?}
    SVG -->|no| STOP[Publishing stops]
    SVG -->|yes| HTML{HTML export complete?<br/>no ignored element}
    HTML -->|yes| BOTH[HTML, with SVG<br/>as fallback]
    HTML -->|no or failed| ONLY[SVG only]
    BOTH --> SAN[Allow-list sanitizer]
    ONLY --> SAN
    SAN --> R[Release]
  ",
  document-context: true,
  width: 100%,
)

=== Rendering cache <arch-rendering-cache>

The cache key covers everything rendering depends on: Typst version, template, vendored packages and the exercise tree *except* its assessment data. Fixing a test case therefore recompiles no statement.

Rendered files are part of the release hash. A template change therefore produces a new release, which can be rolled back through the pointer like any other publication.

#decision[
  The rendering cache is content-addressed and kept outside the releases directory, which is pruned at each publication. Each course has its own cache, so one course cannot write ahead of time the output another course will look up (#adr("0023")).
]

=== Accessibility limitation <arch-accessibility-limitation>

Typst vectorizes its glyphs in the SVG: the text there is not selectable, not searchable and not readable by a screen reader. HTML does not have this limitation, but it is not always available.

#decision[
  Markdown remains the default format. Typst is only used for content that cannot be expressed otherwise.
]

#validation[
  The maturity of Typst's HTML export will have to be reassessed with each release. If it becomes sufficient, the SVG fallback can be dropped and the accessibility limitation will disappear.
]

== LaTeX <arch-latex>

An exercise statement may be a `statement.tex`, for instructors who already write LaTeX. No TeX engine runs: #ext("pandoc")[pandoc] converts the body to HTML and MathML at publication time, in the rendering container, then the output goes through the same sanitizer as Typst's.

#decision(id: "ADR-0028")[
  LaTeX is accepted as a subset, converted without executing TeX. Anything pandoc would skip, a preamble, `\input` or TikZ stops publishing and names the exercise and the command, rather than reaching students with missing content.
]

== Mermaid <arch-mermaid>

Diagrams (flowcharts, sequence, class and state diagrams) are written in Mermaid in both formats. In Typst, the course template exposes `mermaid(...)`, as in this documentation. In Markdown, a fenced block tagged `mermaid` is extracted during publishing and wrapped in a one-line Typst document.

#decision(id: "ADR-0016")[
  Mermaid is rendered at publication time by the #ext("merman")[merman] Typst package, in the existing Typst container, to light and dark SVG. No diagram library is sent to the browser: nothing to serve under Safe Exam Browser and no exception to the CSP.
]

#mermaid(
  "
  flowchart LR
    MD[statement.md] --> X[Each fenced mermaid<br/>block extracted]
    X --> W[Wrapped in a one-line<br/>Typst document]
    TY[statement.typ<br/>calls the template's mermaid] --> T
    W --> T[Typst container<br/>merman, course template]
    T --> OK{Supported type<br/>and rendered?}
    OK -->|no| STOP[Publishing stops,<br/>naming the exercise and block]
    OK -->|yes| SVG[Inline SVG, light and dark<br/>Mermaid source as text alternative]
    SVG --> R[Release]
  ",
  document-context: true,
  width: 100%,
)

The Mermaid source is kept as the SVG's text alternative. A diagram type merman does not support, or a diagram that fails to render, stops publishing and names the exercise and the block.

= Internationalization <arch-internationalization>

The platform's interface is multilingual, while the project itself (code, documentation) is written in English only.

#decision(id: "ADR-0011")[
  Every interface string goes through a message key, with one translation file per language. English is the source language; English and French are maintained by the project and kept complete by CI. Other languages are contributed externally and may be partial: a missing key falls back to English. The API and the judge return codes, never sentences, and the interface translates them.
]

Pedagogical content is not translated by the platform: a statement is displayed in the language its author wrote it in.

= Infrastructure <arch-infrastructure>

The infrastructure must be reproducible, versioned and sufficiently independent of manual operations to allow a deployment environment to be rebuilt reliably.

The envisioned architecture distinguishes several levels of responsibility:

#mermaid(
  "
  flowchart TD
    I[Infrastructure]

    I --> T[Terraform]
    I --> VM[ÉTS VM]

    T -.->|Provisioning if available| VM

    VM --> U[Ubuntu LTS]
    A[Ansible] -->|Configuration| U

    U --> SYS[System]
    U --> SEC[Security]
    U --> SVC[Services]

    SVC --> API[API]
    SVC --> J[Judge]

    J --> ISO[gVisor / other isolation mechanisms]
  ",
  document-context: true,
  width: 100%,
)

== Infrastructure provisioning <arch-infrastructure-provisioning>

#ext("terraform")[Terraform] is envisioned to describe and provision infrastructure resources when the hosting environment provides a compatible interface.

It could in particular be used to manage:

- virtual machines;
- networks;
- volumes;
- access rules;
- the resources needed for deployment.

However, the infrastructure provided by the institution may not offer an interface allowing Terraform to create or modify these resources directly.

#decision[
  Terraform will be used when the hosting environment allows infrastructure to be provisioned in an automated way. Otherwise, the VM provided by the institution will be considered a resource external to the project.
]

This distinction avoids introducing Terraform artificially into an environment where it would bring no operational value.

== Operating system <arch-operating-system>

The VMs provided by the institution run #ext("ubuntu")[Ubuntu LTS]. #ext("nixos")[NixOS], initially envisioned (#adr("0003")), is not retained; see #adr("0006").

Ubuntu does not offer native declarative configuration. The system configuration is therefore described by idempotent Ansible playbooks, versioned with the rest of the project, so that a machine can be rebuilt or added from the repository. An inventory by groups (`web`, `judge`, `db`) describes the VMs; adding one amounts to registering it in the inventory and running a playbook (#adr("0010")).

The target model is:

#mermaid(
  "
  flowchart LR
    Git     --> Ansible[Ansible playbooks]
    Ansible --> VM[Ubuntu VM]
    VM      --> A[Reproducible system state]
  ",
  document-context: true,
  width: 100%,
)

The playbooks describe in particular:

- installed packages;
- system services;
- network configuration;
- the firewall;
- the required users and permissions;
- logging mechanisms;
- monitoring services;
- the container runtime;
- the components needed for submission isolation.

#decision[
  The main VM uses Ubuntu LTS, imposed by the institution. Its permanent state is described by the repository's Ansible playbooks.
]

Unlike NixOS, Ubuntu does not keep system generations allowing a return to a previous configuration. This risk is offset by:

- a VM snapshot before each system or platform update;
- pinning the versions of critical packages (container runtime, gVisor, nginx, PostgreSQL);
- limiting `unattended-upgrades` to security fixes, suspended ahead of an exam.

#hypothesis[
  VM snapshots and version pinning can reduce operational risk during platform updates, especially ahead of an assessment period.
]

== Role of Ansible <arch-role-of-ansible>

Ansible is the host configuration mechanism. It covers both the machine's permanent state and one-off operations, for example:

- coordinating an update;
- some deployment operations;
- administrative tasks.

The intended boundary is therefore:

#mermaid(
  "
  flowchart LR
    T[Terraform] --> T1[Infrastructure provisioning]
    A[Ansible]   --> A1[Machine state and operations]
    C[CI/CD]     --> C1[Application build and deployment]
  ",
  document-context: true,
  width: 100%,
)

#decision[
  The system's persistent configuration is declared in the Ansible playbooks. Any manual change to the machine must be carried back into them.
]

Imperative convergence can let the actual state drift from what the repository describes. The playbooks are therefore run regularly in `--check --diff` mode to detect any drift.

== Deployment environment <arch-deployment-environment>

The target infrastructure should ideally be split into several environments when the available resources allow it:

#mermaid(
  "
  flowchart LR
    I[Infrastructure]

    I --> V[Staging]
    I --> P[Production]

    V --> V1[Tests / CI]
    P --> P1[Teaching / exams]
  ",
  document-context: true,
  width: 100%,
)

The staging environment makes it possible in particular to test a new version of the system, the judge or the isolation mechanisms before using it in a real teaching context.

#hypothesis[
  A separate pre-production environment can be particularly useful before exams, since some changes to the judging system or to isolation can have significant consequences on the platform's availability.
]

= CI/CD <arch-ci-cd>

The envisioned pipeline is:

#mermaid(
  "
  flowchart TD
    G[Git push] --> T

    subgraph T[Tests]
      T1[Unit tests]
      T2[Integration tests]
      T3[Security tests]
    end

    T --> B

    subgraph B[Build]
      B1[Web]
      B2[API]
      B3[Judge]
    end

    B --> IMG[Images / artifacts<br/>with build attestation]
    IMG --> REG[Registry / artifact storage]

    subgraph ETS[ÉTS network, through the VPN]
      OP[Ansible run from an<br/>operator workstation]
      VAL[Staging environment]
      HV{Human validation}
      PROD[Production]
    end

    REG -->|attestation verified,<br/>images pulled| OP
    OP --> VAL
    VAL --> HV
    HV -->|approved| PROD
  ",
  document-context: true,
)

The Ansible configuration must itself be tested (`ansible-lint`, runs in `--check` mode) and versioned in the same development cycle.

The pipeline must in particular make it possible to verify that a change to the system or the application can be built before being deployed.

#decision[
  Deployment must be automated as much as possible and reproducible from the repository. Manual changes to production must be avoided or, when necessary, documented and carried back into the declarative configuration.
]

CI/CD must not, however, automatically deploy any change directly to the environment used for exams.

The ÉTS VMs are only reachable through the school's VPN (Cisco Secure Client), which a hosted CI runner cannot join.

#decision[
  CI builds, tests and publishes the images to a registry, with a build attestation that Ansible verifies before pulling them. Deployment is an Ansible run from an operator's workstation connected to the VPN, which pulls those images. A runner inside the ÉTS network, or `ansible-pull` on the VMs, is only considered if manual runs become a burden.
]

A distinction must be maintained between:

* automatic validation;
* deployment to pre-production;
* human validation;
* deployment to production.

== Updates without interruption <arch-updates-without-interruption>

An admin who deploys most likely does it to fix a problem, so the fix must take effect within minutes. During an exam, a deployment is refused unless the admin forces it explicitly, as with `git push --force`. Most components can change version without interrupting anyone, provided each one waits for its own idle moment.

#decision(id: "ADR-0024")[
  A deployment refuses to start while an exam is in progress or starts within 30 minutes, unless run with `-e force=true`, which leaves a `deploy.forced` event. It publishes the desired version of each component and restarts nothing passive. An outdated judge leaves after 10 s without a job, one at a time per VM, and at the latest at the end of its job after 2 minutes; a run is never cut short. The API changes hands between two instances behind nginx, the web interface changes on the next reload, and migrations are expand/contract so two versions can run side by side. Only disruptive steps show the maintenance notice.
]

#table(
  columns: (3cm, 1fr),
  stroke: 0.5pt,
  [*Component*], [*Switches to the new version*],
  [Judges], [When idle for 10 s, one per VM at a time; at the end of their job after 2 minutes],
  [Pre-started sandboxes], [Replaced when idle],
  [API], [New instance behind nginx; the old one finishes its requests; SSE reconnects with `Last-Event-ID`],
  [Web interface], [On the next reload; the previous release's files stay served],
  [nginx], [Graceful `reload`],
  [Dashboard, scaler], [Restarted directly; they hold no job],
  [Publisher], [At the next publication],
  [PostgreSQL restart, heavy migration, reboot of a single instance], [Disruptive: maintenance notice],
)

= Observability <arch-observability>

Diagnosing an ongoing exam relies on three mechanisms carried over from #ext("ctester")[CTester], none of which adds a service to the VMs.

== Dashboard <arch-dashboard>

A separate, read-only application shows the judges, the queue, the served release and, for the instructors of an offering, the runs and per-exercise statistics (#adr("0019")).

== Logs <arch-logs>

Every component writes one JSON object per line to stderr, following the #ext("otel-logs")[OpenTelemetry Logs data model], with no SDK and no collector: journald and Docker keep them. Event names (`job.enqueued`, `job.reclaimed`, `db.unavailable`…) are stable identifiers, since queries depend on them. A collector can read these lines later without any change.

#decision[
  No identity enters a log: no account, name, password, token, address, request body or code. Attributes outside an allow-list are dropped, a stack trace keeps its frames but not the exception's message, and a request is logged by its route template, never its path. A test scans every logging call.
]

This protects the passwords that transit through the API (#adr("0017")) and keeps logs, which a data deletion does not reach, outside the scope of #ext("law25")[Law 25].

== Maintenance notice <arch-maintenance-notice>

While a flag file exists on the `web` VMs, every open tab shows that an update is under way, then that the service is back once the API answers again. The notice goes through the existing SSE stream. A flag older than thirty minutes is ignored, so an interrupted run cannot leave the notice up for good. The Ansible deployment raises the flag in a `block` and removes it in `always`.

#decision[
  Only a disruptive step raises the flag: a PostgreSQL restart, a migration that takes a heavy lock, or a reboot or runtime upgrade on a role with a single instance. Passive components update themselves without a notice (#arch("updates-without-interruption")[Updates without interruption]). A deployment refuses to start while an exam is in progress or starts within 30 minutes (#adr("0012")), unless forced with `-e force=true` (#adr("0024")).
]

#mermaid(
  "
  flowchart TD
    S[Ansible run from an operator<br/>workstation on the VPN] --> E{Exam in progress<br/>or within 30 min?}
    E -->|yes, not forced| X0[Refused]
    E -->|no, or forced| D{Disruptive step?}
    D -->|no| P[Passive components switch<br/>at their own idle moment, no notice]
    D -->|yes| F[Flag raised on the web VMs<br/>block]
    F --> N[Every open tab shows the notice<br/>through the SSE stream]
    N --> X[Disruptive step]
    X --> R[Flag removed<br/>always]
    R --> B[Tabs show the service is back<br/>once the API answers]
  ",
  document-context: true,
)

= Code organization <arch-code-organization>

A monorepo is currently preferred in order to keep a consistent view of the project's various components.

The layout only contains what the MVP needs:

```text
.devcontainer/        local services (PostgreSQL, LDAP) and tools, gVisor included
apps/
├── admin/            read-only dashboard for operations and instructors
├── api/              axum: sign-in, sessions, offerings, submissions, SSE
├── judge/            pulls jobs, runs the sandbox, writes the verdict
├── publisher/        validation, projection, releases, pointer; typst/ template
└── web/              Svelte interface built by Vite, served by nginx; locales/ (en, fr)
db/migrations/        PostgreSQL schema: queue, offerings, sessions, submissions
examples/content/     fake content repository for development
infrastructure/
└── ansible/          inventory/ (local, ets) and roles/ (one per service)
packages/             shared library crates: the API never depends on the judge, nor the reverse
├── contracts/        JSON Schemas and their Rust types: job, verdict report, pack manifest, exercise
└── content/          active release reader and the single "is it open?" rule
packs/                declarative extension points, not core code
├── languages/        one directory per language pack (python, java first)
├── runners/          test runners (stdio first)
└── question-types/   Q1 item schemas
spikes/               throwaway study code, deleted once its ADR is decided
tests/                cross-component tests: architecture, conformance, load
docs/  report/  site/  .github/
Cargo.toml            workspace of every crate in apps/ and packages/
rust-toolchain.toml   Rust version, read by rustup everywhere
```

The `infrastructure/ansible/` directory contains the configuration of the machines administered by the project as well as the administrative operations. A `terraform/` directory is only added if the hosting environment offers a provisioning interface.

Language packs sit outside `apps/judge/`: adding one is adding data, not changing the judge (#adr("0013")). The opening rule lives in `packages/content/` because the API, the judge and the publisher must apply the same one.

#mermaid(
  "
  flowchart LR
    subgraph APPS[apps]
      API[api]
      JUDGE[judge]
      PUB[publisher]
    end
    subgraph PKG[packages]
      CON[contracts<br/>JSON Schemas, Rust types]
      CNT[content<br/>release reader, opening rule]
    end
    subgraph PACKS[packs]
      LANG[languages]
      RUN[runners]
      QT[question-types]
    end
    API --> CON
    API --> CNT
    JUDGE --> CON
    JUDGE --> CNT
    PUB --> CON
    PUB --> CNT
    JUDGE -.->|reads as data| PACKS
    API -.-|never depend on each other| JUDGE
  ",
  document-context: true,
  width: 100%,
)

A dependency between the API and the judge, or from a shared crate to an application, makes the architecture test fail; it reads the crate graph rather than the imports (#adr("0014"), #adr("0025")). Packs are data read at run time, never compiled into the core.

#decision[
  The monorepo is preferred in order to keep a consistent version of the application, the judge engine and the infrastructure. Pedagogical content lives in its own repository (#adr("0004")); `examples/content/` only serves development.
]

Boundaries between components must nevertheless remain explicit.

A monorepo does not mean that all components share the same code, the same language or the same deployment cycle.

= Security <arch-security>

== Defense in depth <arch-defense-in-depth>

Execution security does not rely on the operating system or on a single isolation mechanism.

An execution should ideally cross several layers of protection:

#mermaid(
  "
  flowchart LR
    C[Student code]             --> V[Application validation]
    V                           --> J[Judge]
    J                           --> R[Resource limits]
    R                           --> RT[Container / runtime]
    RT                          --> ISO[gVisor / microVM]
    ISO                         --> H[Ubuntu host]
    H                           --> VM[Institution VM]
  ",
  document-context: true,
  width: 100%,
)

Each layer must reduce the potential consequences of a failure in another layer.

The role of Ubuntu and Ansible in this architecture is mainly to provide a reproducible and manageable system environment. They do not by themselves constitute the isolation boundary for student programs.

== Separation of secrets <arch-separation-of-secrets>

The process responsible for running student code should not have access to the application's critical secrets.

In particular, the judge engine should run with the minimum privileges needed to operate.

A possible compromise of an execution environment must therefore not directly yield the credentials giving access to the platform's critical services.

== Separation of responsibilities on the VM <arch-separation-of-responsibilities-on-the-vm>

When the available resources allow it, components with different trust levels should be separated.

A possible model is:

#mermaid(
  "
  flowchart TB
    subgraph VM[Ubuntu VM]
        API[Application<br/>API / Web]
        J[Judge]

        subgraph ISO[Isolation]
            S[Sandbox]
            C[Student code]
        end

        J --> S
        S --> C
    end

    API -->|Submission| J
  ",
  document-context: true,
  width: 100%,
)

A stronger physical or virtual separation between the application and the judge engine may be considered if the threat analysis or load constraints justify it.

#validation[
  The distribution of roles (`web`, `judge`, `db`) across the VMs (#adr("0010")) will have to be determined based on the available resources, the threat model and the results of the load tests.
]

== Hostile content author <arch-hostile-content-author>

A content author can push anything to their course's repository and holds the `instructor` or `ta` role in their offerings. The platform cannot stop them from harming their own course, but it must stop them from reaching anything else: other courses, the platform, the ÉTS accounts of their students.

#table(
  columns: (4.2cm, 1fr),
  stroke: 0.5pt,
  [*Attack surface*], [*Control (#adr("0023"))*],
  [Statement HTML and SVG (script, fake sign-in form)],
  [Allow-list sanitizer after Typst and merman; strict CSP with `form-action 'self'`],

  [Files in `public/`], [Separate origin without cookies, `attachment`, `nosniff`; short-lived URL signed by the API after the access gate],
  [Symbolic links, submodules, identifiers such as `../`],
  [Tree read from Git objects and refused; course set by the server; paths prefixed by the course],

  [Rendering cache], [One cache per course],
  [Author code (tests, reference, generators, derived keys)],
  [Deferred job in the judge's sandbox, charged to the offering; never in the publisher or the judge process],

  [Formulas and symbolic answers], [Restricted grammar, never evaluated as code],
  [Judge configuration], [Versioned schema; pack and options from enumerations; limits capped by the pack],
  [Private tests in the sandbox], [Only the judged exercise's `assessment/` subtree, read-only],
  [Visible tests in the browser], [Worker in a sandboxed iframe with an opaque origin, no network],
  [Oracle setup script], [Runs as the disposable schema's user, on its own VM and network segment],
  [Re-judging, exam reservations], [Charged to the offering's quota; reservation computed from enrollment],
  [LTI and CSV roles], [LTI context bound by an `admin`; CSV only grants roles below the importer's],
  [Strings shown in the dashboard], [Plain text, inserted with `textContent`],
)

Each publication is traced with the push identity given by the Git host, and an `admin` can freeze a course's publication, pin its pointer and suspend its quota.

== Network <arch-network>

Student code normally has no reason to access the Internet or the institution's internal network.

#decision[
  Network access for student programs must be denied by default. An exercise may only declare loopback networking (a namespace with no external interface), for socket exercises such as #course("LOG100") or #course("GTI611") (#adr("0013")).
]

Network and firewall configuration must be considered part of the declarative infrastructure and not a manual configuration of the machine.

Docker inserts its own rules ahead of ufw: a `ufw deny` on a published port looks effective and does nothing, as CTester found. Rules that concern containers are therefore written in the `DOCKER-USER` chain by the Ansible role, and checked from outside the VM.

== Resources <arch-resources>

Each execution must have explicit limits concerning in particular:

- CPU time;
- total execution time;
- memory;
- the number of processes and threads;
- temporary disk space;
- network connections;
- accessible files.

These limits must be enforced at the level of the isolated execution mechanism and not only by the student's program.

= Reproducibility <arch-reproducibility>

The overall goal of the infrastructure is to be able to answer the question:

> "Can a working and sufficiently identical environment be rebuilt
> from the project's repository?"

The target level of reproducibility is:

#mermaid(
  "
  flowchart LR
    G[Git repository] --> SRC[Application source code]
    G            --> INF[Infrastructure]

    INF --> N[Ansible configuration]
    N   --> VM
    VM  --> SVC[Configured services]
    SVC --> APP[Application]
  ",
  document-context: true,
  width: 100%,
)

Reproducibility does not necessarily mean that every piece of production data must be rebuilt from scratch. Persistent data, secrets and some resources provided by the institution are external dependencies that must be explicitly identified.

#decision[
  The infrastructure must document its external dependencies so that a working configuration does not depend on implicit knowledge held only by an administrator.
]

= Decisions to validate <arch-decisions-to-validate>

The following choices remain conditional or will have to be confirmed experimentally:

#table(
  columns: (3.2cm, 5cm, 1fr),
  stroke: 0.5pt,

  [*Topic*], [*Current position*], [*Validation*],

  [Main OS], [Ubuntu LTS (imposed by the institution)], [gVisor compatibility with the provided kernel],

  [Provisioning], [Terraform if a compatible API is available], [Actual capabilities of the ÉTS environment],

  [Configuration],
  [Ansible playbooks run over the VPN],
  [Drift detection, VM snapshots, `sudo` access, VPN for a service account],

  [Authentication],
  [LDAP bind against the ÉTS Active Directory (#adr("0017"))],
  [LDAPS endpoint and attributes, no password in logs],

  [Build or reuse], [Port CTester's judge (#adr("0018"))], [Java and Python judged under exam load (V-0018)],

  [Runtime], [Docker or Podman], [Compatibility with the isolation mechanism],

  [Isolation], [gVisor initially envisioned], [Benchmark and security tests],

  [Isolation alternative], [Firecracker], [Comparative benchmark],

  [Topology], [Several VMs replicable by Ansible (#adr("0010"))], [Load, security and available resources],

  [Reverse proxy], [nginx; `upstream` if several `web` VMs], [TLS handling by the institution],

  [Queue], [PostgreSQL (`SKIP LOCKED`)], [Exam load tests],

  [Scheduling policy],
  [Protected exams, base priority per mode, aging, usage penalty (#adr("0022"))],
  [Exam p95 wait and no starvation with one student flooding the queue],

  [Judge capacity],
  [Floor and cap per judge VM, floor raised before exams (#adr("0021"))],
  [Rise to the cap and back to the floor with no run cut short; reserved judges up before the exam],

  [Server language],
  [Rust for every server component (#adr("0025"))],
  [Successor's confirmation; first end-to-end path within its milestone],

  [Updates],
  [Passive per component, notice for disruptive steps only (#adr("0024"))],
  [No run cut short, judges converged within 2 minutes, no notice for a routine release, refused during an exam unless forced],

  [Cache], [No dedicated service; judge-side cache], [Profiling of a submission's cost],

  [Performance measurement],
  [Instruction counting under QEMU (#adr("0007"))],
  [Determinism under load and compatibility with gVisor],

  [Visible tests],
  [Browser for easy languages (#adr("0008"))],
  [Load reduction, discrepancies with the judge, Safe Exam Browser],

  [Internationalization],
  [Translation files; English and French complete, others fall back to English (#adr("0011"))],
  [CI check on `fr`/`en` keys, fallback, language under SEB],

  [Question types],
  [Grader families, tiers Q1–Q3, mixed exams (#adr("0015"))],
  [Mixed exam under SEB, same draw after restart, no key in the projection],

  [Diagrams],
  [Mermaid through merman at publication time (#adr("0016"))],
  [Supported diagram types, equivalence between Markdown and Typst],

  [Group schedules],
  [Dates per group, closed by default (#adr("0020"))],
  [Same activity open for one group and closed for another, through the API and the judge],

  [Hostile content],
  [Content is untrusted, confined to its offering (#adr("0023"))],
  [Hostile content repository fixture: every case refused or neutralized],
)

The architecture will be considered stable only after validation of the hypotheses that have a significant impact on the system's security, performance or operability.

= Open questions <arch-open-questions>

Several important questions are deliberately left open.

1. Is PostgreSQL sufficient as a submission queue under an exam load?
2. Which scheduling policy minimizes perceived latency during an
  exam? A first answer is given in #adr("0022"), whose parameters load tests set.
3. How many workers are needed for 1250 students in concurrent exams (#arch("target-load")[Target load])? The cap per judge VM is measured for this (#adr("0021")).
4. How many resources should be allocated to each submission?
5. What is the real cost of gVisor for realistic compilations?
6. Under what conditions does Firecracker become preferable to gVisor?
7. Which strategy effectively limits resource-exhaustion
  attacks?
8. How fine-grained should the language abstraction be?
9. How should language-specific dependencies be managed?
10. How much state should be persisted in PostgreSQL, and for how long? The retention period for submissions, results and logs follows Quebec's #ext("access-act")[_Access to Information Act_] and the ÉTS retention schedule (#ext("archives-act")[_Archives Act_]); it remains to be confirmed with ÉTS. Purging relies on autovacuum and, if the volume justifies it, on partitioning by date rather than on `VACUUM FULL`.
11. How can recovery be guaranteed after the failure of a worker, a VM or the PostgreSQL primary during an exam? An approach is proposed in #adr("0010").
12. What observability is needed to diagnose an ongoing exam? A first answer is given in #arch("observability")[Observability] and #adr("0019"); it remains to be checked during a load test.
13. How can Moodle and Safe Exam Browser be integrated cleanly? For the MVP, the platform is allowed in the instructor's `.seb` file; SEB verification (#adr("0009")) is deferred, and the handoff from Moodle to the platform through LTI remains to be specified.
14. Which part of the architecture should be common to the different courses? A first answer is given by #adr("0012") and #adr("0014"); LOG121 will test it.
15. How is assessment data distributed to the judges when they are spread over several machines: shared mount, copy at publication time or versioned artifact? Whatever the answer, a sandbox only receives the judged exercise's `assessment/` subtree (#adr("0023")).
16. How is performance graded? A first answer is given by the levels of #adr("0007"), with one reference per exercise; to confirm with the instructor.
17. Is code that does not pass all tests measured? If the final submission fails while an earlier check passed, which one is measured? One student in the #arch("appendix-student-survey")[survey] asks that the submission passing the most tests be kept, or that the student choose.
18. Which languages does each course support, and which of them can run in the browser? A first inventory is given in the #arch("appendix-course-inventory")[appendix] and the tiers in #adr("0013"); it remains to be confirmed with each course coordinator.
19. Should existing Moodle question banks be imported (Moodle XML, GIFT), and for which families? A first answer is given in #adr("0029"), which also imports Word statements. CTester's importer is a starting point: it converts Moodle XML and reports what it cannot express instead of dropping it. The grader families (#adr("0015")) cover more types than CTester, such as calculated, essay and image drag and drop.
20. How do TAs grade manual items (essay, UML diagram, file upload) during a heavy exam period: per item across students, or per student?
21. Do ÉTS Moodle spaces match one course group each, or merge several groups? In the first case the LTI context gives the group; in the second, group membership comes from the CSV import (#adr("0020")).
22. Does Moodle accept #ext("ags")[AGS] grade updates after an activity has closed? Grading and performance measurement finish after the session (#adr("0022")); a locked grade item or a grade overridden in the Moodle gradebook would reject them. To confirm with the ÉTS Moodle administrators.
23. Where are the content repositories hosted (GitHub, a GitLab at ÉTS), who can push to them, and does the host expose the identity of each push? The publication log relies on it, since a commit's author field can be forged (#adr("0023")).
24. Which APIs and languages does #course("LOG645") use (OpenMP, MPI, threads; C or C++), how many cores does an exercise need, and is the speedup graded or only the result? #adr("0036") assumes C and C++ with OpenMP and MPI on one judge.
25. What does "local" completion mean for the instructor: computed in the student's browser, as #adr("0035") assumes, or hosted at ÉTS rather than by a vendor? Is completion allowed in exams? In the #arch("appendix-student-survey")[student survey], 36 of 48 students want it in exams and 3 prefer without; the default stays the instructor's call.
26. For the assistant's hints (#adr("0031")): what default cost list, per item or per activity, and may an instructor write some levels by hand instead of the model, which would remove the risk of a leak for those levels? In the #arch("appendix-student-survey")[student survey], the median price students would pay for a first hint is 0.5 point out of 10, and 16 of 49 would pay nothing, far below the example of 1, 3 and 10 points. They ask that rephrasing the statement and explaining a compiler error be free, and they worry about paying for a wrong answer, whereas a charge is final. Exams come last among the activities where they want the assistant (15 of 50).

= Validation methodology <arch-validation-methodology>

Important decisions must come with a technical justification and, when possible, an experimental validation.

The preferred design cycle is:

#mermaid(
  "
  flowchart LR
    P[Problem]        --> H[Hypothesis]
    H                 --> C[Design]
    C                 --> I[Implementation]
    I                 --> E[Experiment]
    E                 --> M[Measurement]
    M                 --> A[Analysis]
    A                 --> D[Decision]
  ",
  document-context: true,
  width: 100%,
)

This approach avoids choosing a technology solely on the basis of its theoretical characteristics.

It also makes it possible to turn some parts of the project into measurable contributions within the special project.

= Planned evolution of the document <arch-planned-evolution-of-the-document>

This document is a first snapshot of the architecture.

The following sections should gradually be completed with:

- functional and non-functional requirements;
- a formal threat model;
- the ADRs;
- protocols between components;
- the definition of the judge engine's interfaces;
- the benchmark methodology;
- experimental results;
- decisions made following the experiments;
- identified limitations;
- conclusions.

= Appendix: course inventory <arch-appendix-course-inventory>

Courses the platform is designed for, in priority order: LOG200, then LOG121, then the others on request. DEG codes come from the #ext("deg-planning")[DEG course planning] published by ÉTS; languages are to be confirmed with each coordinator.

#table(
  columns: (1.6cm, 1fr, 3.6cm, 3.4cm),
  stroke: 0.5pt,
  [*Course*], [*Title*], [*Languages / tools*], [*Test format*],
  table.cell(colspan: 4)[*Software and IT engineering (LOG/GTI)*],
  [#course("LOG200")],
  [Structures de données et algorithmes],
  [Full catalog (#adr("0013"))],
  [Standard I/O, performance],
  [#course("LOG121")], [Conception orientée objet], [Java], [Unit tests, design questions],
  [#course("LOG100")],
  [Programmation et réseautique en génie logiciel],
  [C, Python, sockets],
  [Standard I/O, loopback network],
  [#course("LOG210")], [Analyse et conception de logiciels], [Java, TypeScript], [Unit tests],
  [#course("LOG240")], [Tests et maintenance], [Java, TypeScript], [Unit tests, coverage],
  [#course("LOG635")], [Systèmes intelligents et algorithmes], [Python (#ext("numpy")[NumPy])], [Unit tests],
  [#course("LOG645")], [Architectures de calculs parallèles], [C, C++ with OpenMP and MPI (#adr("0036"))], [Standard I/O, speedup],
  [#course("LOG660")], [Bases de données de haute performance], [SQL], [SQL],
  [#course("GTI350")], [Conception et évaluation des interfaces utilisateurs], [JavaScript], [Unit tests],
  [#course("GTI611")], [Réseaux de communication IP], [C, Python], [Loopback network],
  table.cell(colspan: 4)[*General studies department (DEG)*],
  [#course("INF111")], [Programmation orientée objet], [Java], [Unit tests],
  [#course("INF130")], [Ordinateurs et programmation], [VBA (VB.NET judged)], [Standard I/O],
  [#course("INF136")], [Introduction à la programmation en Python], [Python], [Standard I/O, browser],
  [#course("INF147")], [Programmation procédurale], [C89], [Standard I/O, unit tests],
  [#course("INF155")], [Introduction à la programmation], [C99], [Standard I/O],
  [#course("INF270")], [Programmation Web pour le design UX], [HTML, CSS, JavaScript], [Unit tests],
  [#course("TCH009")], [Informatique], [C], [Standard I/O],
  [#course("TCH016")], [Systèmes d'exploitation et services Internet], [Bash, PowerShell], [Standard I/O],
  [#course("TCH017")], [Architecture des ordinateurs], [Pep/8 assembly], [Standard I/O],
  [#course("TCH055")], [Bases de données], [Oracle SQL], [SQL (shared Oracle)],
  [#course("TCH056")], [Programmation Web], [JavaScript, TypeScript], [Unit tests],
  [#course("TCH057")], [Applications mobiles], [Java, Kotlin], [Unit tests, no emulator],
  [#course("TCH099")], [Projet intégrateur en informatique], [JavaScript, PHP, Java, Kotlin], [Practice exercises],
)

Out of scope: courses without judged code (#course("GTI510"), #course("LOG410"), #course("LOG430"), #course("LOG795")), mobile emulation, Windows commands and Windows Server roles, and multi-service projects.

= Appendix: LOG200 student survey <arch-appendix-student-survey>

An anonymous #ext("log200-survey")[survey] was sent to #course("LOG200") students from 3 to 5 November 2026. It received 50 answers: 47 students taking the course this session, 2 who had taken it and 1 who will. Not every student answered every question, so totals vary. These answers inform the decisions; they do not make them. Defaults that change what an exam measures stay with the instructor.

*Languages* (#adr("0013")).

#table(
  columns: (1fr, auto, auto, auto, auto, auto, auto, auto, auto, auto, auto),
  stroke: 0.5pt,
  [], [Java], [Python], [C\#], [TS], [C++], [JS], [C], [Rust], [Go], [Kotlin],
  [Wanted for exercises], [39], [31], [20], [13], [10], [9], [8], [7], [4], [2],
  [Single exam language], [23], [14], [4], [2], [2], [–], [–], [–], [2], [2],
)

One student chose assembly as their exam language.

*Comprehension assistant* (#adr("0031")).

- Would use it, from 1 to 4: 8, 15, 15 and 12 answers (median 3).
- Where it should be available: practice 39, competitive programming 38, graded assignments 24, exams 15, nowhere 3.
- Points they would give up for a first hint on a 10-point item: 0 for 16 students, 0.5 for 13, 1 for 16, 2 for 4 (median 0.5).
- Help wanted: an idea for the algorithm 31, a compiler error 25, the statement 25, a failing test 23, performance 13.
- Free-text answers ask that rephrasing the statement and explaining a compiler error cost nothing, that a hint the student finds wrong not cost points, and that hints be capped so that students cannot exhaust them in a row.

*Completion* (#adr("0035")).

#table(
  columns: (1fr, auto, auto, auto),
  stroke: 0.5pt,
  [], [Wanted], [Indifferent], [Rather without],
  [Practice], [37], [11], [2],
  [Exams], [36], [9], [3],
)

*Exercise types* (#adr("0015"), #adr("0026"), #adr("0027"), #adr("0032")).

#table(
  columns: (1fr, auto, auto, auto),
  stroke: 0.5pt,
  [], [Very useful], [Somewhat], [Not useful],
  [Predict the output or the complexity], [25], [15], [6],
  [Self-assessment against the revealed solution], [24], [13], [11],
  [Spaced review of theory], [22], [24], [2],
  [Find and fix the bug], [20], [23], [4],
  [Write tests that expose a faulty program], [16], [21], [11],
  [Parsons problem], [7], [19], [21],
)

*Working environment.*

- Devices for practice: personal Windows computer 39, Linux 17, ÉTS lab computers 14, macOS 6, phone 6, tablet 3.
- Usual editor: VS Code 24, a JetBrains IDE 13, Visual Studio 7, Zed 3, others 3.
- Settings in use: dark theme 38, zoom or large text 11, keyboard-only navigation 4, a font adapted for dyslexia 3, a screen reader 1; none 7.
- Interface language: indifferent 24, French 19, English 7.
- The ÉTS VPN: installed but rarely used 27, would discourage practice from home 10, never installed 7, already used regularly 6.

*Frustrations with current tools* (Moodle, CodeRunner under #ext("seb")[Safe Exam Browser], paper exams). Copy and paste is by far the most cited: it fails in the Moodle editor, and under SEB a multi-line paste needs an extra key press and keeps only its first two lines. Next come the lack of completion and of error underlining, compiler messages that do not point at the right line, hidden tests that fail without any clue, no undo after submitting, no middle-click scrolling under SEB, and writing code on paper.
