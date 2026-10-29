#import "../template.typ": adr, arch, ext, validation

== ADR-0025 — Server implementation language <adr-0025>

*Status:* proposed. Replaces the decision of the section #arch("implementation-language")[Implementation language], which was not an ADR and wrote the judge in Python like the application. Applies #adr("0014"). \
*See also:* #arch("purpose-of-the-document")[architecture], sections #arch("implementation-language")[Implementation language] and #arch("code-organization")[Code organization]; #adr("0018").

=== Context

Python was chosen for the judge and the application so that the platform would have one server language, for its handover to the instructor who takes it over and to the ÉTS IT service. That argument rests on the people who will maintain it:

#table(
  columns: (1fr, 2.6cm, 2.6cm),
  stroke: 0.5pt,
  [*Language*], [*Author*], [*Successor*],
  [#ext("rust")[Rust]], [1st], [3rd],
  [C], [2nd], [—],
  [Python], [3rd], [4th (last)],
  [Java], [4th (last)], [2nd],
  [C++], [Rejected], [1st],
)

Python is last for the successor and third for the author, who builds the platform alone. Rust is the only language both rank above Python.

The judge is also the only privileged process: it drives Docker, mounts the content of an exercise and receives hostile input, from student code, from course content (#adr("0023")) and possibly from a compromised API. #ext("ctester")[CTester] replaced its Python worker with a Rust judge for this reason: `#![forbid(unsafe_code)]`, `openat2` without following links, its own check of whether an exercise is open, a strict systemd unit. Speed is not the argument: a submission spends its time in the sandbox and the compiler, not in the orchestrator.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Python everywhere (current)],
  [Fastest start; #ext("fastapi")[FastAPI] and libraries for LTI and LDAP; CTester's API reusable.],
  [Last choice of the successor; a privileged judge in a dynamically typed language.],

  [Java everywhere],
  [Second choice of the successor; common language of the LOG courses.],
  [Last choice of the author; one JVM per on-demand judge (#adr("0021")) on small VMs; nothing reusable from CTester.],

  [C++ for the judge],
  [First choice of the successor.],
  [Rejected by the author; not memory-safe in the process that receives hostile input.],

  [Rust judge, Python application (CTester's split)],
  [Both parts of CTester reusable; memory-safe judge.],
  [Two server languages; the opening rule implemented twice and bound by shared test vectors.],

  [Rust everywhere],
  [Ranked above Python by both maintainers; one server language; memory safety by construction; single binaries; CTester's judge reusable.],
  [No LTI 1.3 library; CTester's API not reusable; fewer students know Rust.],
)

=== Decision

All server code is written in Rust, in one #ext("cargo")[Cargo] workspace pinned to Rust 1.99.1 by `rust-toolchain.toml`. `unsafe` code is forbidden in the whole workspace.

- *API*: #ext("axum")[axum] on #ext("tokio")[tokio], PostgreSQL through #ext("sqlx")[sqlx], LDAPS through the `ldap3` crate (#adr("0017")).
- *Judge*: synchronous; an asynchronous runtime is only added if a measurement justifies it.
- *Publisher and dashboard*: Rust as well. Typst and merman keep running in their own sandboxed container (#adr("0023")), whatever the publisher's language.
- *Shared code*: `packages/content` (release reader, opening rule) and `packages/contracts` (JSON Schemas and their Rust types) are library crates used by the applications.

No other server language is added. Python remains only as Ansible's runtime; the web interface is unchanged.

=== Consequences

- One opening rule, in `packages/content`, shared by the API, the judge and the publisher, rather than two implementations kept in step.
- Component boundaries are read from the crate graph: a crate can only use what its `Cargo.toml` declares. A CI script fails if the API and the judge depend on each other, or if a shared crate depends on an application (#adr("0014")).
- The VMs run binaries built and attested by CI, with no language runtime to install (#arch("ci-cd")[CI/CD]).
- CTester's judge becomes a candidate for reuse (#adr("0018")); its FastAPI application does not.
- LTI 1.3 (OIDC launch, JWT and JWKS validation) is written in the project, with a general JWT crate. The integration stays optional (#arch("open-questions")[open question 13]).
- Fewer student contributors know Rust than Java, C or Python; the operations guide and the conformance suites carry more of the onboarding.

#validation(id: "V-0025")[
  The successor confirms their ranking. The first end-to-end path (submission, queue, sandbox, verdict) is delivered within its milestone in the schedule. `systemd-analyze security` rates the judge's unit at least as well as CTester's.
]
