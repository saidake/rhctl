#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Initialize an installed PostgreSQL for remote access and create a test DB + role.
#
# Each run mints a new random database name / role / password (preset formats;
# random preset if unset) and writes them to the state file (latest wins for
# execute-sql.sh). Server listen / port / pg_hba / firewall changes stay idempotent.
# Remote pg_hba rules (exact allow-list; re-runs overwrite):
#   - default / omit --allowed-ips: 0.0.0.0/0 and ::/0
#   - with --allowed-ips: only those client addresses (same as limit-remote-ips.sh)
#   - reopen after restrict: --allowed-ips 0.0.0.0/0,::/0
#
# Idempotent for server config — safe to re-run. Credentials are always new.
#
# Usage:
#   ./init.sh \
#     --host 192.168.75.129 \
#     --port 5432 \
#     --auth-method md5 \
#     --allowed-ips 192.168.1.100,10.0.0.5 \
#     --credential-profile hardened
#   ./init.sh \
#     --host 192.168.75.129 \
#     --port 5432 \
#     --auth-method md5 \
#     --allowed-ips 0.0.0.0/0,::/0 \
#     --credential-profile hardened
#   ./init.sh \
#     --port 5433 \
#     --auth-method scram-sha-256 \
#     --credential-profile app_snake
#
# Required Parameters:
#   (none)
#
# Optional Parameters:
#   --host <host>
#       Address printed in DATABASE_URL / psql test (default: `127.0.0.1`).
#       Use the target server IP/hostname when clients connect remotely.
#         Example host values: `192.168.75.129`, `db.example.com`, `127.0.0.1`
#   --port <port>
#       Listen port (default: `5432`).
#         Example port values: `5432`, `5433`, `48985`
#   --auth-method <method>
#       pg_hba auth method (default: `md5`).
#         Example method values: `md5`, `scram-sha-256`, `password`
#   --allowed-ips <ip>[,<ip>...]
#       Desired final remote allow-list (overwrites prior remote host-all rules;
#       omit for `0.0.0.0/0,::/0`). Use `0.0.0.0/0,::/0` to allow all again.
#         Example ip values: `192.168.1.100`, `10.0.0.5`, `192.168.1.0/24`,
#           `0.0.0.0/0`, `::/0`
#   --credential-profile <profile>
#       Bundle of name + password formats. If unset (and formats unset), a profile is
#       chosen at random so installs do not share one pattern.
#         Example profile values: `dev_simple`, `dev_hex`, `app_snake`, `hardened`
#   --db-name-format <format>
#       Database identifier format (Postgres-safe unquoted name).
#         Example format values / generated names:
#           `p_alnum`   → `p7k2m9xq4n1b`
#           `u_hex`     → `u0a1b2c3d4e5f678`
#           `app_alnum` → `app_k3m9x2p7q1`
#           `db_snake`  → `db_a1b2c3d4_e5f6`
#           `r_digit`   → `rabcd12345678`
#   --user-name-format <format>
#       Role identifier format (same allowed values as `--db-name-format`).
#         Example format values / generated names:
#           `p_alnum`   → `p9xq4n1b7k2m`
#           `u_hex`     → `ufedcba9876543210`
#           `app_alnum` → `app_z8y7x6w5v4`
#           `db_snake`  → `db_m1n2o3p4_q5r6`
#           `r_digit`   → `rwxyz87654321`
#   --password-format <format>
#       Password generation format.
#         Example format values / generated passwords:
#           `alnum24`     → `K7mP2qR9tX4vB8nH1jL5wY3`
#           `alnum32`     → `A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6`
#           `hex48`       → `a1b2c3d4e5f6789012345678abcdef0123456789abcdef01`
#           `alnum_sym28` → `K7mP2q#R9tX4v*B8nH_1jL5wY3z`
#           `base58_32`   → `3fK9mP2qR7tX4vB8nH1jL5wY6zA2cD`
#
# Override Parameters:
#   RHCTL_PG_HOST=<host>
#       Same as `--host`.
#   RHCTL_PG_PORT=<port>
#       Same as `--port`.
#   RHCTL_PG_AUTH_METHOD=<method>
#       Same as `--auth-method`.
#   RHCTL_PG_ALLOW_IPS=<ip>[,<ip>...]
#       Same as `--allowed-ips`.
#   RHCTL_PG_CREDENTIAL_PROFILE=<profile>
#       Same as `--credential-profile`.
#   RHCTL_PG_DB_NAME_FORMAT=<format>
#       Same as `--db-name-format`.
#   RHCTL_PG_USER_NAME_FORMAT=<format>
#       Same as `--user-name-format`.
#   RHCTL_PG_PASSWORD_FORMAT=<format>
#       Same as `--password-format`.
#   RHCTL_PG_STATE_FILE=<path>
#       Latest credentials state file (overwritten each init run).
#         Example path values: `/var/lib/postgresql/.rhctl-pg-test-credentials`
#
# Since : 1.0.1
# Date  : Sep 26, 2026
# ************************************************************************************

set -euo pipefail

# Empty means "not set" so we can fall back to state-file DB_HOST on re-runs.
RHCTL_PG_HOST="${RHCTL_PG_HOST:-}"
RHCTL_PG_PORT="${RHCTL_PG_PORT:-5432}"
RHCTL_PG_AUTH_METHOD="${RHCTL_PG_AUTH_METHOD:-md5}"
STATE_FILE="${RHCTL_PG_STATE_FILE:-/var/lib/postgresql/.rhctl-pg-test-credentials}"
CREDENTIAL_PROFILE="${RHCTL_PG_CREDENTIAL_PROFILE:-}"
DB_NAME_FORMAT="${RHCTL_PG_DB_NAME_FORMAT:-}"
USER_NAME_FORMAT="${RHCTL_PG_USER_NAME_FORMAT:-}"
PASSWORD_FORMAT="${RHCTL_PG_PASSWORD_FORMAT:-}"
NEED_RESTART=false
IPS=()

NAME_FORMATS=(p_alnum u_hex app_alnum db_snake r_digit)
PASSWORD_FORMATS=(alnum24 alnum32 hex48 alnum_sym28 base58_32)
PROFILES=(dev_simple dev_hex app_snake hardened)

while [ "$#" -gt 0 ]; do
    case "$1" in
        --host)
            RHCTL_PG_HOST="$2"
            shift 2
            ;;
        --port)
            RHCTL_PG_PORT="$2"
            shift 2
            ;;
        --auth-method)
            RHCTL_PG_AUTH_METHOD="$2"
            shift 2
            ;;
        --allowed-ips)
            IFS=',' read -r -a _parsed <<<"$2"
            IPS+=("${_parsed[@]}")
            shift 2
            ;;
        --credential-profile)
            CREDENTIAL_PROFILE="$2"
            shift 2
            ;;
        --db-name-format)
            DB_NAME_FORMAT="$2"
            shift 2
            ;;
        --user-name-format)
            USER_NAME_FORMAT="$2"
            shift 2
            ;;
        --password-format)
            PASSWORD_FORMAT="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '2,110p' "$0"
            exit 0
            ;;
        --*)
            echo "[ERROR] Unknown option: $1"
            exit 1
            ;;
        *)
            IPS+=("$1")
            shift
            ;;
    esac
done

if [ "${#IPS[@]}" -eq 0 ] && [ -n "${RHCTL_PG_ALLOW_IPS:-}" ]; then
    IFS=',' read -r -a IPS <<<"${RHCTL_PG_ALLOW_IPS}"
fi

case "$RHCTL_PG_AUTH_METHOD" in
    md5|scram-sha-256|password|trust|reject) ;;
    *)
        echo "[ERROR] Unsupported --auth-method: ${RHCTL_PG_AUTH_METHOD}"
        echo "[ERROR] Allowed: md5, scram-sha-256, password, trust, reject"
        exit 1
        ;;
esac

log() { echo "[INFO] $*"; }
warn() { echo "[WARN] $*"; }

in_list() {
    local needle="$1"
    shift
    local x
    for x in "$@"; do
        [ "$x" = "$needle" ] && return 0
    done
    return 1
}

pick_random() {
    local arr=("$@")
    local n=${#arr[@]}
    local i=$((RANDOM % n))
    echo "${arr[$i]}"
}

apply_profile() {
    case "$1" in
        dev_simple)
            DB_NAME_FORMAT="${DB_NAME_FORMAT:-p_alnum}"
            USER_NAME_FORMAT="${USER_NAME_FORMAT:-p_alnum}"
            PASSWORD_FORMAT="${PASSWORD_FORMAT:-alnum24}"
            ;;
        dev_hex)
            DB_NAME_FORMAT="${DB_NAME_FORMAT:-u_hex}"
            USER_NAME_FORMAT="${USER_NAME_FORMAT:-u_hex}"
            PASSWORD_FORMAT="${PASSWORD_FORMAT:-hex48}"
            ;;
        app_snake)
            DB_NAME_FORMAT="${DB_NAME_FORMAT:-db_snake}"
            USER_NAME_FORMAT="${USER_NAME_FORMAT:-app_alnum}"
            PASSWORD_FORMAT="${PASSWORD_FORMAT:-alnum32}"
            ;;
        hardened)
            DB_NAME_FORMAT="${DB_NAME_FORMAT:-db_snake}"
            USER_NAME_FORMAT="${USER_NAME_FORMAT:-r_digit}"
            PASSWORD_FORMAT="${PASSWORD_FORMAT:-alnum_sym28}"
            ;;
        *)
            echo "[ERROR] Unsupported --credential-profile: $1"
            echo "[ERROR] Allowed: ${PROFILES[*]}"
            exit 1
            ;;
    esac
}

# Resolve credential formats: explicit flags win; else random profile / random formats.
if [ -n "$CREDENTIAL_PROFILE" ]; then
    apply_profile "$CREDENTIAL_PROFILE"
elif [ -z "$DB_NAME_FORMAT" ] && [ -z "$USER_NAME_FORMAT" ] && [ -z "$PASSWORD_FORMAT" ]; then
    CREDENTIAL_PROFILE="$(pick_random "${PROFILES[@]}")"
    apply_profile "$CREDENTIAL_PROFILE"
    log "No credential formats specified — randomly selected profile: ${CREDENTIAL_PROFILE}"
else
    CREDENTIAL_PROFILE="${CREDENTIAL_PROFILE:-custom}"
fi

[ -z "$DB_NAME_FORMAT" ] && DB_NAME_FORMAT="$(pick_random "${NAME_FORMATS[@]}")"
[ -z "$USER_NAME_FORMAT" ] && USER_NAME_FORMAT="$(pick_random "${NAME_FORMATS[@]}")"
[ -z "$PASSWORD_FORMAT" ] && PASSWORD_FORMAT="$(pick_random "${PASSWORD_FORMATS[@]}")"

if ! in_list "$DB_NAME_FORMAT" "${NAME_FORMATS[@]}"; then
    echo "[ERROR] Unsupported --db-name-format: ${DB_NAME_FORMAT}"
    echo "[ERROR] Allowed: ${NAME_FORMATS[*]}"
    exit 1
fi
if ! in_list "$USER_NAME_FORMAT" "${NAME_FORMATS[@]}"; then
    echo "[ERROR] Unsupported --user-name-format: ${USER_NAME_FORMAT}"
    echo "[ERROR] Allowed: ${NAME_FORMATS[*]}"
    exit 1
fi
if ! in_list "$PASSWORD_FORMAT" "${PASSWORD_FORMATS[@]}"; then
    echo "[ERROR] Unsupported --password-format: ${PASSWORD_FORMAT}"
    echo "[ERROR] Allowed: ${PASSWORD_FORMATS[*]}"
    exit 1
fi

rand_chars() {
    local charset="$1"
    local len="$2"
    local out
    # `head` closes the pipe early; under `pipefail` that yields SIGPIPE (141) from `tr`.
    set +o pipefail
    out="$(tr -dc "$charset" </dev/urandom | head -c "$len")"
    set -o pipefail
    if [ "${#out}" -ne "$len" ]; then
        echo "[ERROR] Failed to generate ${len} random characters" >&2
        exit 1
    fi
    printf '%s' "$out"
}

# Unquoted Postgres identifiers: [a-z_][a-z0-9_]* , max 63 chars.
generate_identifier() {
    case "$1" in
        p_alnum)  echo "p$(rand_chars 'a-z0-9' 12)" ;;
        u_hex)    echo "u$(rand_chars 'a-f0-9' 16)" ;;
        app_alnum) echo "app_$(rand_chars 'a-z0-9' 10)" ;;
        db_snake) echo "db_$(rand_chars 'a-z0-9' 8)_$(rand_chars 'a-z0-9' 4)" ;;
        r_digit)  echo "r$(rand_chars 'a-z' 4)$(rand_chars '0-9' 8)" ;;
        *)
            echo "[ERROR] Internal: unknown identifier format $1" >&2
            exit 1
            ;;
    esac
}

generate_password() {
    case "$1" in
        alnum24)     rand_chars 'A-Za-z0-9' 24 ;;
        alnum32)     rand_chars 'A-Za-z0-9' 32 ;;
        hex48)       rand_chars 'a-f0-9' 48 ;;
        # Avoid shell/SQL/URL metacharacters: ' " ` $ \ and whitespace.
        alnum_sym28) rand_chars 'A-Za-z0-9!@#%^*_-' 28 ;;
        base58_32)   rand_chars '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz' 32 ;;
        *)
            echo "[ERROR] Internal: unknown password format $1" >&2
            exit 1
            ;;
    esac
}

sql_quote() {
    # Escape single quotes for SQL string literals.
    printf "%s" "$1" | sed "s/'/''/g"
}

url_encode() {
    if command -v python3 &>/dev/null; then
        python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$1"
    else
        # Fallback: only safe when password is already URL-safe.
        printf "%s" "$1"
    fi
}

if ! command -v psql &>/dev/null; then
    echo "[ERROR] psql not found. Run install.sh first."
    exit 1
fi

PG_VERSION=$(psql --version 2>&1 | grep -oP '(?<=psql \(PostgreSQL\) )[\d.]+' | head -1 | cut -d. -f1)
CONF_DIR="/etc/postgresql/${PG_VERSION}/main"
CONF_FILE="${CONF_DIR}/postgresql.conf"
HBA_FILE="${CONF_DIR}/pg_hba.conf"

if [ ! -f "$CONF_FILE" ] || [ ! -f "$HBA_FILE" ]; then
    echo "[ERROR] Expected config under ${CONF_DIR} (version=${PG_VERSION})"
    exit 1
fi

set_conf_kv() {
    local key="$1"
    local value="$2"
    local file="$3"
    local expected="${key} = ${value}"
    local current

    current=$(grep -E "^[[:space:]]*${key}[[:space:]]*=" "$file" | head -1 || true)
    if [ -n "$current" ]; then
        local normalized
        normalized=$(echo "$current" | sed 's/^[[:space:]]*//;s/[[:space:]]*=[[:space:]]*/ = /;s/[[:space:]]\+/ /g')
        if [ "$normalized" = "$expected" ]; then
            log "postgresql.conf '${key}' already set to ${value}"
            return 1
        fi
        sudo sed -i -E "s|^[[:space:]]*${key}[[:space:]]*=.*|${expected}|" "$file"
        log "Updated postgresql.conf '${key}' = ${value}"
        return 0
    fi

    if grep -qE "^[[:space:]]*#[[:space:]]*${key}[[:space:]]*=" "$file"; then
        sudo sed -i -E "s|^[[:space:]]*#[[:space:]]*${key}[[:space:]]*=.*|${expected}|" "$file"
        log "Uncommented postgresql.conf '${key}' = ${value}"
        return 0
    fi

    echo "$expected" | sudo tee -a "$file" >/dev/null
    log "Appended postgresql.conf '${key}' = ${value}"
    return 0
}

normalize_cidr() {
    local ip="$1"
    if [[ "$ip" != */* ]]; then
        echo "${ip}/32"
    else
        echo "$ip"
    fi
}

remote_hba_snapshot() {
    sudo grep -E '^host[[:space:]]+all[[:space:]]+all[[:space:]]+' "$HBA_FILE" 2>/dev/null \
        | grep -Ev '[[:space:]](127\.0\.0\.1/32|::1/128)[[:space:]]' || true
}

remove_remote_hba_rules() {
    sudo sed -i -E \
        '/^host[[:space:]]+all[[:space:]]+all[[:space:]]+(127\.0\.0\.1\/32|::1\/128)[[:space:]]+/b
         /^host[[:space:]]+all[[:space:]]+all[[:space:]]+/d' \
        "$HBA_FILE"
}

# Replace remote host-all rules with the desired CIDR list. Returns 0 if changed.
sync_remote_hba_allowlist() {
    local -a desired=("$@")
    local before after cidr line
    before="$(remote_hba_snapshot)"
    remove_remote_hba_rules
    for cidr in "${desired[@]}"; do
        line="host    all             all             ${cidr}               ${AUTH}"
        echo "$line" | sudo tee -a "$HBA_FILE" >/dev/null
        log "pg_hba allow: ${cidr} (${AUTH})"
    done
    after="$(remote_hba_snapshot)"
    if [ "$before" != "$after" ]; then
        return 0
    fi
    return 1
}

configure_firewall() {
    local port="$1"
    if ! command -v ufw &>/dev/null; then
        log "ufw not installed — skipping firewall"
        return 0
    fi
    if sudo ufw status 2>/dev/null | grep -qE "(${port}/tcp|${port}\\s)"; then
        log "Firewall rule for ${port}/tcp already exists"
        return 0
    fi
    sudo ufw allow "${port}/tcp" comment 'PostgreSQL (rhctl)'
    log "Allowed ${port}/tcp in ufw"
}

# --- listen / port ---
if set_conf_kv "listen_addresses" "'*'" "$CONF_FILE"; then NEED_RESTART=true; fi
if set_conf_kv "port" "${RHCTL_PG_PORT}" "$CONF_FILE"; then NEED_RESTART=true; fi

AUTH="$RHCTL_PG_AUTH_METHOD"

# --- remote auth (exact allow-list; re-runs overwrite) ---
DESIRED_CIDRS=()
if [ "${#IPS[@]}" -gt 0 ]; then
    for raw in "${IPS[@]}"; do
        ip="$(echo "$raw" | xargs)"
        [ -z "$ip" ] && continue
        DESIRED_CIDRS+=("$(normalize_cidr "$ip")")
    done
else
    DESIRED_CIDRS=("0.0.0.0/0" "::/0")
fi

if [ "${#DESIRED_CIDRS[@]}" -eq 0 ]; then
    echo "[ERROR] --allowed-ips produced an empty allow-list"
    exit 1
fi

if sync_remote_hba_allowlist "${DESIRED_CIDRS[@]}"; then NEED_RESTART=true; fi

configure_firewall "$RHCTL_PG_PORT"

# --- credentials: always mint a new random DB + role each run ---
DB_CONNECT_HOST="${RHCTL_PG_HOST:-127.0.0.1}"
DB_NAME="$(generate_identifier "$DB_NAME_FORMAT")"
DB_USER="$(generate_identifier "$USER_NAME_FORMAT")"
DB_PASSWORD="$(generate_password "$PASSWORD_FORMAT")"

# Extremely unlikely with random formats; regenerate if a name already exists.
tries=0
while [ "$tries" -lt 8 ]; do
    role_exists=$(sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='${DB_USER}'" || true)
    db_exists=$(sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'" || true)
    if [ "$role_exists" != "1" ] && [ "$db_exists" != "1" ]; then
        break
    fi
    [ "$role_exists" = "1" ] && DB_USER="$(generate_identifier "$USER_NAME_FORMAT")"
    [ "$db_exists" = "1" ] && DB_NAME="$(generate_identifier "$DB_NAME_FORMAT")"
    tries=$((tries + 1))
done
if [ "$tries" -ge 8 ]; then
    echo "[ERROR] Failed to allocate unique database/role names after ${tries} attempts"
    exit 1
fi

sudo mkdir -p "$(dirname "$STATE_FILE")"
sudo tee "$STATE_FILE" >/dev/null <<EOF
DB_NAME=${DB_NAME}
DB_USER=${DB_USER}
DB_PASSWORD=${DB_PASSWORD}
DB_HOST=${DB_CONNECT_HOST}
DB_PORT=${RHCTL_PG_PORT}
DB_AUTH_METHOD=${AUTH}
DB_CREDENTIAL_PROFILE=${CREDENTIAL_PROFILE}
DB_NAME_FORMAT=${DB_NAME_FORMAT}
DB_USER_NAME_FORMAT=${USER_NAME_FORMAT}
DB_PASSWORD_FORMAT=${PASSWORD_FORMAT}
EOF
sudo chmod 600 "$STATE_FILE"
sudo chown postgres:postgres "$STATE_FILE" 2>/dev/null || true
log "Generated new credentials → ${STATE_FILE}"
log "Formats: profile=${CREDENTIAL_PROFILE} db=${DB_NAME_FORMAT} user=${USER_NAME_FORMAT} password=${PASSWORD_FORMAT}"

DB_PASSWORD_SQL="$(sql_quote "$DB_PASSWORD")"
DB_PASSWORD_URL="$(url_encode "$DB_PASSWORD")"

sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE USER ${DB_USER} WITH PASSWORD '${DB_PASSWORD_SQL}';"
log "Created role '${DB_USER}'"

sudo -u postgres psql -v ON_ERROR_STOP=1 -c "CREATE DATABASE ${DB_NAME} OWNER ${DB_USER};"
log "Created database '${DB_NAME}' owned by '${DB_USER}'"

if [ "$NEED_RESTART" = true ]; then
    log "Restarting PostgreSQL to apply config"
    sudo systemctl restart postgresql
else
    log "No postgresql.conf / pg_hba changes requiring restart"
fi

if ! systemctl is-active --quiet postgresql; then
    warn "PostgreSQL service is not active — attempting start"
    sudo systemctl start postgresql
fi

echo "============================================="
echo "[INFO] PostgreSQL test credentials"
echo "[INFO]   DB_NAME=${DB_NAME}"
echo "[INFO]   DB_USER=${DB_USER}"
echo "[INFO]   DB_PASSWORD=${DB_PASSWORD}"
echo "[INFO]   DB_HOST=${DB_CONNECT_HOST}"
echo "[INFO]   DB_PORT=${RHCTL_PG_PORT}"
echo "[INFO]   DB_AUTH_METHOD=${AUTH}"
echo "[INFO]   CREDENTIAL_PROFILE=${CREDENTIAL_PROFILE}"
echo "[INFO]   DB_NAME_FORMAT=${DB_NAME_FORMAT}"
echo "[INFO]   USER_NAME_FORMAT=${USER_NAME_FORMAT}"
echo "[INFO]   PASSWORD_FORMAT=${PASSWORD_FORMAT}"
if [ "${#IPS[@]}" -gt 0 ]; then
    echo "[INFO]   ALLOW_IPS=${DESIRED_CIDRS[*]}"
else
    echo "[INFO]   ALLOW_IPS=0.0.0.0/0 ::/0"
fi
echo "[INFO]   DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD_URL}@${DB_CONNECT_HOST}:${RHCTL_PG_PORT}/${DB_NAME}"
echo "[INFO] Test:"
echo "[INFO]   PGPASSWORD='${DB_PASSWORD}' psql -U ${DB_USER} -d ${DB_NAME} -h ${DB_CONNECT_HOST} -p ${RHCTL_PG_PORT}"
echo "============================================="
