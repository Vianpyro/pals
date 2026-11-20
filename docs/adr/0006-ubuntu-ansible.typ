#import "../template.typ": adr, arch, ext, validation

== ADR-0006 — Ubuntu and Ansible for VM configuration <adr-0006>

*Status:* accepted. Supersedes #adr("0003"). \
*See also:* #arch("purpose-of-the-document")[architecture], section #arch("infrastructure")[Infrastructure].

=== Context

The institution provides a VM running #ext("ubuntu")[Ubuntu LTS], 26.04 as announced by the ÉTS system administrator in November 2026; #ext("nixos")[NixOS] (#adr("0003")) is not retained. The infrastructure must nevertheless be rebuildable from the repository, and the #ext("ctester")[CTester] failures caused by configuration spread across several repositories must not happen again.

=== Options considered

#table(
  columns: (3cm, 1fr, 1fr),
  stroke: 0.5pt,
  [*Option*], [*Pros*], [*Cons*],
  [Ubuntu + #ext("ansible")[Ansible] in the monorepo],
  [Well known, supported by the institution; already used for CTester; versioned with the application.],
  [Imperative convergence: the actual state can drift; no native system rollback.],

  [Ubuntu + container images only],
  [Reproducible application.],
  [The host (runtime, #ext("gvisor")[gVisor], firewall) is still configured by hand.],
)

=== Decision

Ubuntu LTS, configured by idempotent Ansible playbooks versioned in the monorepo. Ansible holds all of the host's permanent state: packages, container runtime, gVisor (`runsc` from the official apt repository), firewall, #ext("nginx")[nginx] and #ext("certbot")[certbot], users, logging.

=== Consequences

- A single source of truth for a machine's state: the repository's playbooks. Any manual change is carried back into them.
- Drift is detected by regularly running the playbooks in `--check --diff` mode.
- Without system generations, rollback relies on a VM snapshot before each update and on pinned package versions.
- `unattended-upgrades` is limited to security fixes and suspended ahead of an exam.

#validation(id: "V-0006")[
  Confirm with the ÉTS infrastructure team that VM snapshots and `sudo` access for Ansible are available; check that gVisor works on the provided Ubuntu kernel.
]
