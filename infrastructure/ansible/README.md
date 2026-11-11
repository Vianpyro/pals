# Deployment

One playbook builds the whole platform on the ÉTS VMs, and rerunning it changes nothing (ADR-0006, ADR-0010). It runs from an operator's machine connected to the ÉTS VPN, because the VMs cannot be reached any other way. No CI job deploys.

```
ansible-playbook -i inventory/ets site.yml
```

## What runs where

| Group | Roles | Holds |
|---|---|---|
| `db` | common, postgresql | PostgreSQL 18 (PGDG repository), a nightly dump in `/var/backups/log791` kept for 14 days |
| `web` | common, nginx, content, api | nginx on 443 (HTTPS only), the page, the releases, the API on 127.0.0.1:8080 |
| `judge` | common, docker, gvisor, content, judge | Docker with runsc, the releases and the raw content with its hidden tests, `log791-judge@N` |

For the MVP, a single VM can belong to every group.

On the operator's machine, the `artifacts` role does two things:
- It verifies the build provenance of each image that `images.yml` pushed, then extracts the files the image holds. The VMs receive only these files: nothing on them pulls from GitHub.
- It publishes each course's content at the revision named in the inventory. Releases (statements only) go to the web and judge VMs. The raw content, hidden tests included, goes to the judges only, readable by `log791-judge` alone.

## The operator's machine

The dev container works. Otherwise, a Linux machine with:
- the versions of `.devcontainer/requirements.txt`, then `ansible-galaxy collection install -r requirements.yml` from this directory;
- `docker` and `git`;
- `gh`, signed in with `gh auth login`;
- `docker login ghcr.io`, if the images are private.

## Before the first run

1. In `inventory/ets/hosts.yml`, list the VMs the ÉTS provides.
2. In `inventory/ets/group_vars/all/main.yml`, set:
   - `log791_version`, an image tag such as `0.1.46`;
   - the courses, each at a tag or commit of its content repository;
   - the server name;
   - the directory settings: an `ldaps://` URL only, and the CA that signs the directory's certificate, which is the only one the API trusts (ADR-0017).
3. Create the vault. Use passwords without URL characters, for example from `openssl rand -hex 32`:

   ```
   ansible-vault create inventory/ets/group_vars/all/vault.yml
   ```

   ```yaml
   vault_database_owner_password: ...
   vault_database_api_password: ...
   vault_database_judge_password: ...
   vault_ldap_bind_password: ...
   # PEM, when nginx_tls_source is files
   nginx_tls_certificate: |
     -----BEGIN CERTIFICATE-----
   nginx_tls_certificate_key: |
     -----BEGIN PRIVATE KEY-----
   ```

4. Run `ansible-playbook -i inventory/ets site.yml --ask-vault-pass`.

## Still open

- **The certificate.** By default (`nginx_tls_source: files`), the certificate and key come from the vault. certbot cannot validate a host reachable only through the VPN over HTTP, and whether the ÉTS terminates TLS itself is still to be confirmed.
## Everyday operations

| To | Run |
|---|---|
| Deploy a new version | Change `log791_version`, then run `site.yml` |
| Publish a course's new content | Change the course's `version`, then run `site.yml`. Nothing restarts. |
| Replace an offering's roster | `ansible-playbook -i inventory/ets enroll.yml -e offering=log200-a26 -e roster=roster.csv` (CSV `account,role`; whoever is missing loses their role; the `offerings` row must exist) |
| Check what would change | `ansible-playbook -i inventory/ets site.yml --check --diff` |
| Check that nothing drifted | `bash drift.sh`: the same check, which fails if any VM differs from `site.yml` |
| Before an exam period | `ansible-playbook -i inventory/ets pre-exam.yml`: stops the automatic upgrades, pulls every pack image, then checks disk space, the judges, last night's backup and the API |
| After an exam period | `ansible-playbook -i inventory/ets post-exam.yml`: turns the automatic upgrades back on |

The API migrates the database before it restarts. The judges restart afterwards: a job stopped halfway is taken again once its lease expires (ADR-0001).

## Testing a change

`bash tests/deploy/run.sh [24.04|26.04]` checks a change, from the dev container:
1. It builds the images.
2. It deploys `site.yml` twice onto three Ubuntu containers, and fails if the second run changes anything.
3. It enrolls a roster with `enroll.yml`.
4. It has a submission judged through nginx.
5. It restores the nightly backup into a scratch database and compares its rows.
6. It fails if the `systemd-analyze security` exposure of the API or the judge gets worse.
7. It runs `drift.sh`, `pre-exam.yml` and `post-exam.yml`.

CI runs it on Ubuntu 24.04 when a change concerns it, and nightly on 24.04 and 26.04.
