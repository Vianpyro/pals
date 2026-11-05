#import "../template.typ": adr, arch, ext, validation

== ADR-0019 — Operations and teaching dashboard <adr-0019>

*Status:* proposed; carried over from #ext("ctester")[CTester], where it is in production. \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("observability")[Observability]; #adr("0001"), #adr("0012").

=== Context

During an exam, someone must be able to tell within seconds whether judges are alive, how long the queue is and which release is served, and an instructor wants to see how an exercise is going. CTester answers this with a small, separate, read-only dashboard, used during every TCH009 lab. It shows the live workers, the queue (waiting, running, oldest job, estimated wait), the published revision, the history of runs, per-exercise statistics and the number of open windows, refreshed every five seconds.

=== Options considered

- *Pages inside the student API*: nothing more to deploy, but the student-facing process gains routes that read every student's code, and it cannot be exposed separately.
- *A generic monitoring stack* (metrics server, dashboards): answers the operations question only, and adds several services to VMs with limited resources.
- *A separate read-only application*, as in CTester.

=== Decision

A separate application, `apps/admin/`, connected to #ext("postgresql")[PostgreSQL] with a read-only role. It can be exposed on the VPN only, independently of the student site. It never writes a verdict, a grade or a student's row.

- *Two views, by role* (#adr("0012")). The operations view (judges, queue, served release, failures) is for `admin`. The teaching view (runs, per-exercise statistics, submitted code) is for the `instructor` and `coordinator` of an offering, restricted to that offering.
- *The queue is the journal.* In CTester, the judge appends a JSON line per run and the dashboard ingests it, because its queue is a directory. Here the queue is a table (#adr("0001")): every run stays in it, including cache hits, reclaimed and abandoned jobs, and the dashboard reads it directly. No journal file, no ingestion.
- *Live updates* through #ext("sse")[server-sent events] on a five-second tick, one query per tick for all open tabs; the stream ends periodically so that reconnecting rechecks the session. nginx buffering is disabled for that route (`X-Accel-Buffering: no`).
- *Names and code hidden by default.* Student names and code are only sent when explicitly asked for, for the projector and people looking over a shoulder. Code is inserted with `textContent` under a strict #ext("csp")[CSP], since the page holds a privileged session.
- *Open windows* are counted from recent session activity in PostgreSQL, not from process memory, since the API runs several processes.

=== Consequences

- One more process to deploy, with its own Ansible role and its own database role.
- The dashboard answers #arch("open-questions")[open question 12] for the queue, the judges and the release; logs cover the rest (#arch("observability")[Observability]).
- Runs are kept in the queue table; their retention follows the offering (#adr("0012")).

#validation(id: "V-0019")[
  During a load test, stopping a judge shows it as dead within one job, and the queue's oldest job and estimated wait follow the load. An instructor of LOG121 sees no LOG200 run. With the database role in place, any write from the dashboard fails.
]
