# P.A.L.S - Programming Assessment and Learning System

A platform where students write, run and submit code that is judged in an isolated sandbox, built first for LOG200 at ÉTS. The architecture, the ADRs and the project plan are published at <https://vianpyro.github.io/pals/>; their sources are in [`docs/`](docs/).

## Getting started

The server code is a Cargo workspace in Rust 1.99.1 (ADR-0025), pinned by [`rust-toolchain.toml`](rust-toolchain.toml). Everything runs in the dev container: Rust, PostgreSQL, a local LDAP directory, Docker with gVisor, Typst and Ansible.

1. Open the repository in VS Code and choose **Reopen in Container** (Docker required).
2. Wait for the first build. It fetches the crates (`cargo fetch`), then `check.sh` prints one line per service; every line should read `ok`.
3. Run the checks CI runs: `cargo clippy --all-targets`, `cargo test` and `bash tests/architecture/boundaries.sh`.

Common commands are VS Code tasks (**Tasks: Run Task**):

| Task | What it does |
|------|--------------|
| `rust: check` | Format, Clippy and tests, as CI does |
| `architecture: check` | Check component boundaries in the crate graph, as CI does |
| `env: check` | Check that every local service answers |
| `docs: build site` | Build every document into `_site/` |
| `db: psql` | Open a shell on the local database |
| `gvisor: sandbox shell` | A shell in a gVisor sandbox with no network |

Test accounts for the local directory are in [`.devcontainer/ldap/`](.devcontainer/ldap/); the lldap admin interface is forwarded on port 17170.

Outside the dev container, install [rustup](https://rustup.rs/): it reads `rust-toolchain.toml` and installs the right toolchain on the first `cargo` command. The services and gVisor are then yours to provide.

## Layout

| Path | Contents |
|------|----------|
| `apps/` | `api`, `judge`, `publisher`, `admin` (Rust binaries) and `web` (static) |
| `packages/` | Code shared by the apps: `contracts` (JSON Schemas and their Rust types), `content` (release reader, opening rule) |
| `packs/` | Language packs, test runners and question types: data, not core code |
| `db/migrations/` | PostgreSQL schema |
| `infrastructure/ansible/` | Machine configuration and operations |
| `tests/` | Cross-component tests: architecture, conformance, load |
| `spikes/` | Throwaway study code, deleted once its ADR is decided |
| `docs/`, `report/`, `site/` | Architecture, ADRs, project plan, final report, documentation site |

The full description is in the *Code organization* section of the architecture document.

## Rules

- The API and the judge never depend on each other, and `packages/` depends on no app. `tests/architecture/boundaries.sh` checks the crate graph in CI (ADR-0014).
- No `unsafe` code: the workspace forbids it.
- A language, a course or a question type is added as data under `packs/` or in content, never as a special case in the core.
- An accepted ADR is never edited: a new ADR supersedes it. A new ADR is also listed in [`docs/adr/index.typ`](docs/adr/index.typ).
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/) (see [`.copilot/commit-message-instructions.md`](.copilot/commit-message-instructions.md)). The [`commit-msg`](.githooks/commit-msg) hook refuses the others; the dev container enables it, elsewhere run `git config core.hooksPath .githooks`. CI checks every pushed commit the same way.
- Releases are automatic. After a green CI on `main`, [`release.yml`](.github/workflows/release.yml) bumps the workspace version when the commits call for it (`feat`, `fix`, `perf`, breaking changes) and commits the bump, so run `git pull` afterwards. It then publishes any untagged version as a GitHub release and builds its images. A version raised by hand in `Cargo.toml` is kept. 1.0.0 is always set by hand.
