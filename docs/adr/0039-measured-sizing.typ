#import "../template.typ": adr, arch, validation

== ADR-0039 — Hardware sizing measured in CI <adr-0039>

*Status:* proposed; feeds #adr("0021"), and validates #adr("0001") and #adr("0010"). \
*See also:* #arch("target-load")[architecture], sections #arch("target-load")[Target load] and #arch("sizing")[Sizing].

=== Context

The IT service needs to know which machines to provide, and the #arch("target-load")[target load] rests on rates nobody had measured: 2 s per job, for instance. Judges were assumed to scale: twice the judges, twice the rate.

A first measure on a development machine (16 logical cores, under WSL2, 2026-11-06, exercise of 5 cases) refutes both assumptions:

- A Java job takes 8.7 s alone and 14 s among 16 judges, since each case starts Java and compiles the source again.
- Python stops gaining at 4 judges, about 1.4 jobs per second, with 70% of the CPU idle. About 7 sandboxes per second also bounds sandboxes started without the API, the judge or PostgreSQL: starting them is the limit, not the queue. Whether this holds outside nested Docker under WSL2 remains to be measured.

These figures describe that machine, and only show why judges cannot be sized from a fixed duration.

=== Decision

*A workflow computes a minimum and a recommended configuration per role.* The minimum holds 250 students, the recommended 1250 with one spare judge machine. `tests/load/run.sh` measures one run in the dev container, against a throwaway PostgreSQL; `tests/load/sizing.js` turns the runs into a report. The report of the latest run is the reference: this document holds no configuration, which would age.

*A judge machine is sized from its ceiling.* Bursts measure the rate a machine sustains at each number of judges. The ceiling is the fewest judges within 90% of the best rate; past it, more judges only make each job slower. The number of machines follows from the demand: students $div$ period $div$ target utilization $div$ ceiling. The most limiting language pack sets it. The target utilization is 70%, and also sets the longest wait after a burst, as a share of the period between two submissions: about 2 minutes.

*The queue is modelled, and the model is checked against each run.* The queue is a bounded M/M/c, since a student has one job waiting at most: Erlang C for the steady wait, students $times$ duration $div$ judges for a burst. The report shows the model next to what the `jobs` table measured.

*A figure says how it was obtained.* It is "measured" only when one machine held that load in a steady run; anything else is "extrapolated". The workflow fails only on a memory over its budget (`tests/load/budget.json`), since a shared runner makes times too noisy to fail on.

=== Consequences

- The workflow runs weekly and on demand, not on pull requests: it is long, and its times are noisy.
- A runner cannot reach 1250 students, nor Java's ceiling: these stay extrapolated.
- A judge machine is assumed comparable to the machine measured, whose cores the report states. The ÉTS VMs are reached only through the VPN, so CI cannot measure them: replaying `run.sh` on one is manual.
- Sign-in, nginx and the loss of a machine are not part of the load yet.

#validation(id: "V-0039")[
  Two runs of the workflow on one commit give close memory figures. A deliberate allocation fails the budget. Replayed on an ÉTS VM, the ceilings and the model's waits are compared with those of the runner, and the assumption of comparable machines is corrected accordingly.
]
