#!/usr/bin/env bash
# Seeds lldap from ldap/ (compose service ldap-seed). lldap's bootstrap.sh only reads *.json,
# while jq, which parses them, reads JSON Lines as well: *.jsonl are given to it under a .json name.
set -euo pipefail
shopt -s nullglob

configs=$(mktemp -d)
for kind in user-configs group-configs; do
  mkdir "$configs/$kind"
  for file in "/bootstrap/$kind"/*.json "/bootstrap/$kind"/*.jsonl; do
    # users.json and users.jsonl become users.json.json and users.jsonl.json: no overwrite
    cp "$file" "$configs/$kind/$(basename "$file").json"
  done
done

USER_CONFIGS_DIR="$configs/user-configs" GROUP_CONFIGS_DIR="$configs/group-configs" \
  exec /app/bootstrap.sh
