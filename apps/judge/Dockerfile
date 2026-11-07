# The judge as one binary, with the language packs it was tested with (ADR-0013, ADR-0025). Nothing
# runs from this image: the Ansible judge role copies /judge and /packs onto the judge VMs, where
# systemd runs it and it starts each sandbox with Docker.
# Built and attested by .github/workflows/images.yml from the repository root.
# Bookworm's glibc (2.36) is older than Ubuntu 24.04's (2.39): the binary runs on either LTS.
# Same Rust as rust-toolchain.toml, copied below: if they differ, rustup fetches that one.
FROM rust:1.99.1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN SQLX_OFFLINE=true cargo build --release --locked -p judge

FROM scratch
COPY --from=build /src/target/release/judge /judge
COPY packs/languages /packs
