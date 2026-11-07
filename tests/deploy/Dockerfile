# A stand-in for an ÉTS VM in tests/deploy/run.sh: Ubuntu booted by systemd, with what Ansible
# needs to reach it and nothing the roles are supposed to install.
ARG UBUNTU=24.04
FROM ubuntu:${UBUNTU}

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl iproute2 python3 sudo systemd systemd-sysv \
    && rm -rf /var/lib/apt/lists/*

STOPSIGNAL SIGRTMIN+3
CMD ["/sbin/init"]
