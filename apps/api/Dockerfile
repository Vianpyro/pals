# The API as one binary, with no runtime to install on the VMs (ADR-0025). Nothing runs from this
# image: the Ansible api role copies /api onto the web VMs, where systemd runs it.
# Built and attested by .github/workflows/images.yml from the repository root.
# Bookworm's glibc (2.36) is older than Ubuntu 24.04's (2.39): the binary runs on either LTS.
# Same Rust as rust-toolchain.toml, copied below: if they differ, rustup fetches that one.
FROM rust:1.99.1-slim-bookworm AS build
WORKDIR /src
COPY . .
RUN SQLX_OFFLINE=true cargo build --release --locked -p api

FROM scratch
COPY --from=build /src/target/release/api /api
