---
name: rhctl
description: >-
  Run and author rhctl CLI workflows and remote environment scripts under
  scripts/ (PostgreSQL, JetStream, Docker, Redis, MongoDB, AWS LocalStack,
  execute/upload/patch). Use when the user asks to install/configure remote
  services, mint databases, execute SQL remotely, upload files, or add/edit
  scripts under scripts/.
---

# rhctl

## Script root

```text
SCRIPT_ROOT = C:/Users/saidake/Desktop/DevProjects/rhctl/scripts
```

Absolute path to the repo’s `scripts/` tree. Canonical scripts live only under `$SCRIPT_ROOT`. **Do not copy them into this skill** — always read the live file header before invoking or editing.

Put script path + args in **one** quoted `--script` value. Use `--use-sudo` when the script needs root/`postgres`. Prefer `--identity` when available; `--password` also covers sudo.

## Commands

### rhctl execute

Runs one or more local Bash scripts on a remote host.

```bash
rhctl execute \
  --host <host> \
  --user <user> \
  [--ssh-port <port>] \
  [--password <pass>] \
  [--identity <key>] \
  [--certificate <cert>] \
  --script <script-or-cmdline> \
  [--script <script-or-cmdline> ...] \
  [--work-path <path>] \
  [--mode sync|async] \
  [--use-sudo] \
  [options]
```

Example (script with args):

```bash
rhctl execute \
  --host 192.168.75.129 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/postgresql/db.sh --action create --port 5432" \
  --use-sudo
```

**Required Parameters**:
- `--host <ip/hostname>`: Remote host IP or hostname
- `--user <username>`: Remote username
- `--script <cmdline>`: Local bash script path, optionally followed by args (shell-style quoting). Supports multiple.

**Optional Parameters**:
- `--mode <sync|async>`: `sync` (sequential) or `async` (concurrent)
- `--work-path <path>`: Remote working directory (default: `~`)
- `--password <password>`: SSH password (also sudo / key passphrase fallback)
- `--identity <path>`: SSH private key (preferred over password)
- `--certificate <path>`: OpenSSH certificate (requires `--identity`)
- `--ssh-port <port>`: SSH port (default: `22`)
- `--use-sudo`: Run with sudo (default: false)
- `--use-rsync`: Prefer rsync over scp (default: false)
- `--silent`: Auto-confirm overwrite/delete prompts (default: false)
- `--connect-timeout <duration>`: e.g. `20s`, `5m`, `1h`
- `--max-sessions-per-server <num>` / `--max-channels-per-session <num>`
- `--session-acquire-timeout <duration>` / `--max-session-lifetime <duration>`
- `--debug`: Enable debug logging (default: info)

Path placeholders `${NAME}` expand from the process environment.

### rhctl upload

Upload files/directories to a remote directory via `--transfer` and/or `--transfer-file`.

```bash
rhctl upload \
  --host <host> \
  --user <user> \
  [--transfer <local=remote-dir> ...] \
  [--transfer-file <file>] \
  [options]
```

Transfer format: `<local-path>=<remote-directory>` (RHS is a directory; created if missing). At least one of `--transfer` / `--transfer-file` required. When both set, file loads first; `--transfer` overrides the same local path.

```bash
rhctl upload \
  --host 192.168.75.128 \
  --user test99 \
  --transfer assets/example1.txt=~/examples \
  --transfer assets/exampledir=~/examples/targetdir
```

**Required Parameters**:
- `--host`, `--user`
- At least one of `--transfer <local=remote-dir>` (repeatable) or `--transfer-file <path>`

**Optional Parameters**: Same SSH / sudo / rsync / silent / timeout / session / `--debug` options as `execute`.

### rhctl patch

Upload a local file, backup the remote target, then overwrite (or `--recover` from backup).

```bash
rhctl patch \
  --host <host> \
  --user <user> \
  --local-path <path> \
  --remote-upload <path> \
  --remote-path <path> \
  --remote-backup <path> \
  [--recover] \
  [options]
```

**Required Parameters**:
- `--host`, `--user`
- `--local-path <path>`: Local source file
- `--remote-upload <path>`: Remote upload destination
- `--remote-path <path>`: Remote target to patch
- `--remote-backup <path>`: Backup path before patch

**Optional Parameters**:
- `--recover`: Restore `remote-path` from `remote-backup`
- Same SSH / sudo / rsync / silent / timeout / session / `--debug` options as `execute`

### rhctl run

Run batch upload/execute/patch tasks from a YAML config (servers/groups in parallel).

```bash
rhctl run --config <yml-file-path> --config-name <name>
```

**Required Parameters**:
- `--config <yml-file-path>`
- `--config-name <name>`

For YAML shape and field details, read `README.md` (`rhctl run` section).

## Scripts

### `$SCRIPT_ROOT/postgresql/install.sh`

Install PostgreSQL (latest from PGDG) and ensure the service is running. Default port: 5432. If exists, skip.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/postgresql/install.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/postgresql/configure.sh`

Configure installed PostgreSQL for remote access (listen, port, pg_hba, firewall). Does not create DBs/roles — use `db.sh --action create`. If exists, overwrite.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/postgresql/configure.sh --port 5432" \
  --use-sudo
```

**Required Parameters**: (none)

**Optional Parameters**:
- `--port <port>`: Listen port (default: `5432`). Example: `5432`, `5433`, `48985`
- `--auth-method <method>`: pg_hba auth (default: `md5`). Example: `md5`, `scram-sha-256`, `password`
- `--allowed-ips <ip>[,<ip>...]`: Remote allow-list; re-runs overwrite. When omitted, remote rules are cleared (local `127.0.0.1` / `::1` only). Example: `192.168.1.100`, `10.0.0.5`, `192.168.1.0/24`, `0.0.0.0/0`, `::/0`

**Override Parameters**:
- `RHCTL_PG_PORT`, `RHCTL_PG_AUTH_METHOD`, `RHCTL_PG_ALLOW_IPS` — same as the flags above

### `$SCRIPT_ROOT/postgresql/db.sh`

Manage databases and roles on an already-configured server.

- `create` — mint random DB + role + password; write state file; print URL
- `list` — list non-template databases and owners
- `delete` — drop a database and optionally its owner role

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/postgresql/db.sh --action create --host 192.168.75.128 --port 5432" \
  --use-sudo
```

**Required Parameters**:
- `--action <action>`: `create` | `list` | `delete`

**Optional Parameters**:
- `--host <host>`: Address in DATABASE_URL for `create` (default: `127.0.0.1`)
- `--port <port>`: PostgreSQL port (default: `5432`)
- `--db-name <name>`: Database name for `delete` (required for delete)
- `--user-name <name>`: Role to drop with `delete` (default: database owner)
- `--credential-profile <profile>`: Format bundle for `create` — `dev_simple`, `dev_hex`, `app_snake`, `hardened`
- `--db-name-format <format>`: `p_alnum`, `u_hex`, `app_alnum`, `db_snake`, `r_digit`
- `--user-name-format <format>`: Same values as `--db-name-format`
- `--password-format <format>`: `alnum24`, `alnum32`, `hex48`, `alnum_sym28`, `base58_32`

**Override Parameters**:
- `RHCTL_PG_HOST`, `RHCTL_PG_PORT`, `RHCTL_PG_STATE_FILE`, `RHCTL_PG_CREDENTIAL_PROFILE`, `RHCTL_PG_DB_NAME_FORMAT`, `RHCTL_PG_USER_NAME_FORMAT`, `RHCTL_PG_PASSWORD_FORMAT`

### `$SCRIPT_ROOT/postgresql/execute-sql.sh`

Execute SQL files against a PostgreSQL database. Connection (first match): `DATABASE_URL` → state file from `db.sh --action create` → `PG*` vars.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/postgresql/execute-sql.sh --dir /opt/pidifa/ddl --continue" \
  --use-sudo
```

**Required Parameters**:
- `--dir <directory>`: Run all `*.sql` in the directory (sorted); may repeat. Example: `./sql`, `/opt/pidifa/ddl`

**Optional Parameters**:
- `--continue`: Do not stop on the first SQL error (default: stop)

**Override Parameters**:
- `DATABASE_URL=<url>`: Preferred connection URL
- `RHCTL_PG_STATE_FILE=<path>`: Credentials state file from `db.sh --action create`
- `PGHOST` / `PGPORT` / `PGUSER` / `PGPASSWORD` / `PGDATABASE`: Libpq overrides when `DATABASE_URL` is unset

### `$SCRIPT_ROOT/postgresql/limit-remote-ips.sh`

Set remote pg_hba to an exact client allow-list (keeps localhost). If exists, overwrite.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/postgresql/limit-remote-ips.sh --allowed-ips 192.168.1.100,10.0.0.5 --auth-method md5" \
  --use-sudo
```

**Required Parameters**:
- `--allowed-ips <ip>[,<ip>...]`: Final remote allow-list. Example: `192.168.1.100`, `10.0.0.5`, `192.168.1.0/24`, `0.0.0.0/0`, `::/0`

**Optional Parameters**:
- `--auth-method <method>`: pg_hba auth (default: `md5`). Example: `md5`, `scram-sha-256`, `password`

**Override Parameters**:
- `RHCTL_PG_ALLOW_IPS`, `RHCTL_PG_AUTH_METHOD`

### `$SCRIPT_ROOT/jetstream/install.sh`

Install NATS Server with JetStream and a systemd `nats` service. Default port: 4222. If exists, skip.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/jetstream/install.sh" \
  --use-sudo
```

**Required Parameters**: (none)

**Optional Parameters**:
- `--nats-version <version>`: Release tag when `--archive` unset (default: `latest`; leading `v` added if omitted). Example: `latest`, `v2.15.0`
- `--archive <path>`: Existing `.tar.gz` on the host; skips GitHub download

**Override Parameters**:
- `RHCTL_NATS_VERSION`, `RHCTL_NATS_ARCHIVE`

### `$SCRIPT_ROOT/jetstream/configure.sh`

Configure installed JetStream for remote access; prints NATS URL. If exists, overwrite.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/jetstream/configure.sh --host 192.168.75.128 --port 4222" \
  --use-sudo
```

**Required Parameters**: (none)

**Optional Parameters**:
- `--host <host>`: Address printed in NATS_URL (default: `127.0.0.1`). Use the client-visible IP/hostname for remote clients
- `--port <port>`: Listen port in nats.conf (default: `4222`)

**Override Parameters**:
- `RHCTL_NATS_HOST`, `RHCTL_NATS_PORT`

### `$SCRIPT_ROOT/docker/install.sh`

Install Docker and Docker Compose on the remote host.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/docker/install.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/redis/install.sh`

Install Redis. Port: 6379.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/redis/install.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/redis/apply-test-config.sh`

Apply test Redis config for remote access (`bind 0.0.0.0 ::`, `protected-mode no`, firewall 6379). Idempotent.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/redis/apply-test-config.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/mongodb/install.sh`

Install MongoDB.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/mongodb/install.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/localstack-start.sh`

Start LocalStack (requires Docker/Compose on the remote host). Often preceded by uploading LocalStack files via `rhctl upload`.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/localstack-start.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/localstack-stop.sh`

Stop LocalStack.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/localstack-stop.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/dynamodb-create-tables.sh`

Create/seed LocalStack DynamoDB test tables.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/dynamodb-create-tables.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/dynamodb-delete-tables.sh`

Delete LocalStack DynamoDB test tables.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/dynamodb-delete-tables.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/sns-init.sh`

Create SNS topic, SQS queue, and subscription (LocalStack).

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/sns-init.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/sns-messages.sh`

Receive/delete/test SNS→SQS messages (LocalStack).

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/sns-messages.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/lambda-create-function.sh`

Create LocalStack Lambda function (and IAM bits as needed).

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/lambda-create-function.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/lambda-invoke-function.sh`

Invoke/update LocalStack Lambda function.

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/lambda-invoke-function.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)

### `$SCRIPT_ROOT/aws/ec2-ami.sh`

EC2 AMI / key-pair helper commands (AWS CLI).

```bash
rhctl execute \
  --host 192.168.75.128 \
  --user test99 \
  --password testpwd \
  --script "$SCRIPT_ROOT/aws/ec2-ami.sh" \
  --use-sudo
```

**Required Parameters**: (none)  
**Optional Parameters**: (none)  
**Override Parameters**: (none)
