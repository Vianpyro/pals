# Security policy

P.A.L.S. runs students' code during graded exams. A flaw can let someone escape the sandbox, read hidden tests, or change a grade, so reports are welcome and handled first.

## Reporting a vulnerability

Report it privately through [GitHub's private vulnerability reporting](https://github.com/Vianpyro/pals/security/advisories/new), never in a public issue or pull request. Include:

- what an attacker can do, and as whom (student, instructor, course content author, anonymous);
- the steps or a proof of concept;
- the version (`Cargo.toml`, or the image tag) or commit.

You will get an acknowledgement within 5 working days. A fix is released as a patch version, and the advisory is published once the deployed platform is updated. There is no bug bounty: this is a university project (ÉTS, LOG791).

## Scope

In order of priority:

1. **Sandbox escape**: a submission that reaches the judge VM, the network, another submission or the hidden tests (gVisor, ADR-0002, ADR-0018).
2. **Exam integrity**: reading another student's work or the hidden tests, submitting outside one's schedule, bypassing Safe Exam Browser checks (ADR-0009, ADR-0020).
3. **Untrusted course content**: a content repository that runs code outside its sandbox or reaches another course (ADR-0023).
4. **Authentication and sessions**: LDAP sign-in, sessions, roles (ADR-0012, ADR-0017).
5. **Personal information** (Quebec's Law 25): identity data in logs, in exports, or kept past an offering.
6. **Supply chain**: a released image or binary that does not come from this repository's workflows. Verify one with `gh attestation verify oci://ghcr.io/vianpyro/pals-<app>@<digest> --repo Vianpyro/pals --signer-workflow Vianpyro/pals/.github/workflows/images.yml`.

Out of scope: the [demo](https://vianpyro.github.io/pals/demo/), which runs Python in your own browser with no server, and denial of service by volume.

## What protects a release

- Every image is built by `.github/workflows/images.yml` on GitHub's runners and carries a build provenance attestation and an SPDX SBOM. Ansible refuses an image whose provenance does not match the release tag being deployed.
- A release with a known vulnerability that has a fix is not published (Trivy, `cargo auditable` binaries). Advisories against released images are checked every night.
- Dependencies are pinned: Cargo and npm lockfiles, images by digest, GitHub Actions by commit. Updates wait 7 days before they are proposed, except security fixes.
- CI runs with read-only tokens. Write permissions exist only in the jobs that publish, never on pull requests.
