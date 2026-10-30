#!/usr/bin/env bash
# Checks that every local service and tool answers. Run on every container start;
# safe to rerun by hand: bash .devcontainer/check.sh
set -u

failed=0

# retry SECONDS COMMAND...: services and the inner dockerd start alongside this script.
retry() {
  local tries=$1
  shift
  for _ in $(seq "$tries"); do
    "$@" >/dev/null 2>&1 && return 0
    sleep 1
  done
  return 1
}

check() {
  local name=$1
  shift
  if retry "$@"; then
    echo "ok    $name"
  else
    echo "FAIL  $name"
    failed=1
  fi
}

ldap_finds_student() {
  ldapsearch -x -H "$LDAP_URL" -D "$LDAP_BIND_DN" -w "$LDAP_BIND_PASSWORD" \
    -b "$LDAP_BASE_DN" "($LDAP_USER_ATTRIBUTE=etudiant1)" "$LDAP_USER_ATTRIBUTE" \
    | grep -q "^$LDAP_USER_ATTRIBUTE: etudiant1"
}

runsc_registered() {
  docker info --format '{{json .Runtimes}}' | grep -q runsc
}

check "postgres" 30 pg_isready -h postgres -U log791
check "ldap: test accounts" 60 ldap_finds_student
check "docker: inner daemon" 30 docker info
check "docker: runsc registered" 5 runsc_registered
check "gvisor: hello-world" 1 docker run --rm --runtime=runsc hello-world
check "gvisor: python" 1 docker run --rm --runtime=runsc python:3.14-alpine python -c "print(1)"
check "typst" 1 typst --version
check "ansible" 1 ansible --version
check "ansible-lint" 1 ansible-lint --version
check "cargo" 1 cargo --version
check "gh" 1 gh --version

if [ "$failed" -ne 0 ]; then
  cat <<'EOF'

Some checks failed; everything that passed is usable.
- ldap: see `docker logs` of the ldap-seed service from the host (Docker Desktop).
- gvisor: runsc may not start in Docker-in-Docker on WSL2. Run the gVisor spike
  on the Multipass VM instead; the rest of the container is unaffected.
EOF
  exit 1
fi
