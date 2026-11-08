# Development image: tools only, no application code or dependencies.
# Renovate updates the FROM lines, requirements.txt and the ARG below (.github/renovate.json5).

# Node for the web interface: same version as apps/web/Dockerfile, which Renovate updates with this one
FROM node:26.13.0-trixie-slim AS node

FROM mcr.microsoft.com/devcontainers/python:3.15-trixie

# Also read by .github/workflows/_docs.yml: the HTML export changes between versions.
ARG TYPST_VERSION=0.15.1

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl gnupg ldap-utils postgresql-client xz-utils \
    && curl -fsSL https://gvisor.dev/archive.key | gpg --dearmor -o /usr/share/keyrings/gvisor-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/gvisor-archive-keyring.gpg] https://storage.googleapis.com/gvisor/releases release main" \
    > /etc/apt/sources.list.d/gvisor.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends runsc \
    && rm -rf /var/lib/apt/lists/*

# Read by the dockerd that the docker-in-docker feature starts: the host's Docker
# Desktop cannot be given runsc, the inner daemon can.
RUN mkdir -p /etc/docker \
    && printf '{"runtimes":{"runsc":{"path":"/usr/bin/runsc"}}}\n' > /etc/docker/daemon.json

RUN curl -fsSL "https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-x86_64-unknown-linux-musl.tar.xz" \
    | tar -xJ --strip-components=1 -C /usr/local/bin typst-x86_64-unknown-linux-musl/typst

COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules/npm /usr/local/lib/node_modules/npm
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx

COPY requirements.txt /tmp/requirements.txt
RUN pip install --no-cache-dir -r /tmp/requirements.txt && rm /tmp/requirements.txt
