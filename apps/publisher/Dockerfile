# The publisher as one binary (ADR-0005). Nothing runs from this image: Ansible copies /publisher
# onto the operator's machine and publishes each course's content there, before deploying it.
# Built and attested by .github/workflows/images.yml from the repository root.
# Bookworm's glibc (2.36) is older than Ubuntu 24.04's (2.39) and Debian 13's (2.41).
# Same Rust as rust-toolchain.toml, copied below: if they differ, rustup fetches that one.
FROM rust:1.99.1-slim-bookworm AS build
# The dependency list goes into the binary (cargo auditable), so the scratch image below can be
# scanned for vulnerable crates (images.yml, nightly.yml). Before COPY: rebuilt only with its version.
RUN cargo install cargo-auditable --locked --version 0.7.7
WORKDIR /src
COPY . .
RUN cargo auditable build --release --locked -p publisher

FROM scratch
COPY --from=build /src/target/release/publisher /publisher
