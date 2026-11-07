#!/usr/bin/env bash
# Deploys the platform with infrastructure/ansible/site.yml onto three Ubuntu containers, checks
# that a second run changes nothing, enrolls a roster with enroll.yml, then has a submission
# judged through nginx. Inside the dev container; the containers are deleted at the end.
# Usage: bash tests/deploy/run.sh [24.04|26.04]    KEEP=1 leaves the containers up to look into them
set -euo pipefail
cd "$(dirname "$0")/../.."
root=$PWD

ubuntu=${1:-24.04}
network=log791-test
hosts=(log791-test-web log791-test-judge log791-test-db)
# Not /tmp: a file the containers' root leaves there would block the next run (dev container)
work=$(mktemp -d "$HOME/log791-deploy.XXXXXX")

cleanup() {
  docker rm --force --volumes "${hosts[@]}" >/dev/null 2>&1 || true
  docker network rm "$network" >/dev/null 2>&1 || true
  rm -rf "$work"
}
cleanup
[ -n "${KEEP:-}" ] || trap cleanup EXIT

echo "Building the images, as images.yml does"
for app in api judge publisher web; do
  docker build -q -f "apps/$app/Dockerfile" -t "log795-$app:test" . >/dev/null
done
docker build -q --build-arg "UBUNTU=$ubuntu" -t "log791-test-vm:$ubuntu" tests/deploy >/dev/null

echo "Starting the VMs: Ubuntu $ubuntu"
docker network create "$network" >/dev/null
for host in "${hosts[@]}"; do
  # The judge's own Docker keeps its images in volumes: overlayfs cannot stack on the container's
  docker run -d --name "$host" --hostname "$host" --network "$network" \
    --privileged --cgroupns=host -v /sys/fs/cgroup:/sys/fs/cgroup:rw --tmpfs /run --tmpfs /run/lock \
    -v /var/lib/docker -v /var/lib/containerd "log791-test-vm:$ubuntu" >/dev/null
done

echo "Making the course's content repository"
git init -q -b main "$work/content"
cp -r examples/content/. "$work/content"
git -C "$work/content" add --all
git -C "$work/content" -c user.name=test -c user.email=test@example.invalid commit -q -m "LOG200 content"
export LOG791_TEST_CONTENT=$work/content

cd infrastructure/ansible
ansible-galaxy collection install -r requirements.yml >/dev/null

echo "Deploying"
ansible-playbook -i inventory/local site.yml

echo "Deploying again: nothing may change"
ansible-playbook -i inventory/local site.yml | tee "$work/again.log"
if awk '/^PLAY RECAP/ {recap = 1; next} recap && NF' "$work/again.log" | grep -v ' changed=0 '; then
  echo "FAIL  the second run changed the hosts above" >&2
  exit 1
fi

echo "Enrolling a roster, opening the exercise and a session"
printf 'account,role\netudiant1,student\n' > "$work/roster.csv"
ansible-playbook -i inventory/local enroll.yml -e course=log200 -e "roster=$work/roster.csv"
docker exec log791-test-db sudo -u postgres psql -q -v ON_ERROR_STOP=1 log791 -c "
  INSERT INTO schedules VALUES ('log200', 'sum', now() - interval '1 hour', NULL);
  INSERT INTO sessions (token_hash, account) VALUES (sha256('deploy-test'::bytea), 'etudiant1');"

echo "Submitting through nginx"
api() { docker exec log791-test-web curl -sfk -H 'cookie: __Host-session=deploy-test' "$@"; }
solution=$(<"$root/packs/languages/python/conformance/passed/sum.py")
submission=$(python3 -c 'import json, sys; print(json.dumps({"course": "log200", "exercise": "sum", "language_pack": "python", "source": sys.argv[1]}))' "$solution")
id=$(api -H 'content-type: application/json' -d "$submission" https://localhost/api/submissions | python3 -c 'import json, sys; print(json.load(sys.stdin)["id"])')
for _ in $(seq 60); do
  run=$(api "https://localhost/api/submissions/$id")
  case "$run" in *'"state":"done"'* | *'"state":"failed"'*) break ;; esac
  sleep 2
done
echo "$run"
case "$run" in
  *'"passed":true'*) echo "ok    deployed on Ubuntu $ubuntu: idempotent, and a submission was judged" ;;
  *) echo "FAIL  the submission was not judged as passed" >&2; exit 1 ;;
esac
