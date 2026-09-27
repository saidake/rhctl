#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Manage PostgreSQL databases and roles on an already-configured server.
#
# Actions:
#   create — mint random DB + role + password; write state file; print URL
#   list   — list non-template databases and owners
#   delete — drop a database and optionally its owner role
#
# Usage:
#   ./db.sh --action create --host 192.168.75.129 --port 5432
#   ./db.sh --action list --port 5432
#   ./db.sh --action delete --db-name mydb --user-name myuser --port 5432
#
# Required Parameters:
#   --action <action>
#       Default, operation to run.
#         Example action values: `create`, `list`, `delete`
#
# Optional Parameters:
#   --host <host>
#       Address printed in DATABASE_URL for `create` (default: `127.0.0.1`).
#         Example host values: `192.168.75.129`, `db.example.com`, `127.0.0.1`
#   --port <port>
#       PostgreSQL port (default: `5432`).
#         Example port values: `5432`, `5433`
#   --db-name <name>
#       Database name for `delete` (required for delete).
#         Example name values: `db_a1b2c3d4_e5f6`
#   --user-name <name>
#       Role to drop with `delete` (default: database owner).
#         Example name values: `app_k3m9x2p7q1`
#   --credential-profile <profile>
#       Name + password format bundle for `create`.
#         Example profile values: `dev_simple`, `dev_hex`, `app_snake`, `hardened`
#   --db-name-format <format>
#       Database identifier format for `create`.
#         Example format values / generated names:
#           `p_alnum`   → `p7k2m9xq4n1b`
#           `u_hex`     → `u0a1b2c3d4e5f678`
#           `app_alnum` → `app_k3m9x2p7q1`
#           `db_snake`  → `db_a1b2c3d4_e5f6`
#           `r_digit`   → `rabcd12345678`
#   --user-name-format <format>
#       Role identifier format for `create` (same values as `--db-name-format`).
#   --password-format <format>
#       Password format for `create`.
#         Example format values: `alnum24`, `alnum32`, `hex48`, `alnum_sym28`, `base58_32`
#
# Override Parameters:
#   RHCTL_PG_HOST=<host>
#       Same as `--host`.
#   RHCTL_PG_PORT=<port>
#       Same as `--port`.
#   RHCTL_PG_STATE_FILE=<path>
#       Credentials state file written by `create` (used by execute-sql.sh).
#         Example path values: `/var/lib/postgresql/.rhctl-pg-test-credentials`
#   RHCTL_PG_CREDENTIAL_PROFILE=<profile>
#       Same as `--credential-profile`.
#   RHCTL_PG_DB_NAME_FORMAT=<format>
#       Same as `--db-name-format`.
#   RHCTL_PG_USER_NAME_FORMAT=<format>
#       Same as `--user-name-format`.
#   RHCTL_PG_PASSWORD_FORMAT=<format>
#       Same as `--password-format`.
#
# Since : 1.0.3
# Date  : Sep 27, 2026
# ************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

RHCTL_PG_HOST="${RHCTL_PG_HOST:-}"
RHCTL_PG_PORT="${RHCTL_PG_PORT:-5432}"
STATE_FILE="${RHCTL_PG_STATE_FILE:-/var/lib/postgresql/.rhctl-pg-test-credentials}"
CREDENTIAL_PROFILE="${RHCTL_PG_CREDENTIAL_PROFILE:-}"
DB_NAME_FORMAT="${RHCTL_PG_DB_NAME_FORMAT:-}"
USER_NAME_FORMAT="${RHCTL_PG_USER_NAME_FORMAT:-}"
PASSWORD_FORMAT="${RHCTL_PG_PASSWORD_FORMAT:-}"
ACTION=""
DELETE_DB_NAME=""
DELETE_USER_NAME=""

NAME_FORMATS=(p_alnum u_hex app_alnum db_snake r_digit)
PASSWORD_FORMATS=(alnum24 alnum32 hex48 alnum_sym28 base58_32)
PROFILES=(dev_simple dev_hex app_snake hardened)
PROTECTED_DBS=(postgres template0 template1)
PROTECTED_ROLES=(postgres)

while [ "$#" -gt 0 ]; do
    case "$1" in
        --action)
            ACTION="$2"
            shift 2
            ;;
        --host)
            RHCTL_PG_HOST="$2"
            shift 2
            ;;
        --port)
            RHCTL_PG_PORT="$2"
            shift 2
            ;;
        --db-name)
            DELETE_DB_NAME="$2"
            shift 2
            ;;
        --user-name)
            DELETE_USER_NAME="$2"
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
            sed -n '2,75p' "$0"
            exit 0
            ;;
        --*)
            echo "[ERROR] Unknown option: $1"
            exit 1
            ;;
        *)
            echo "[ERROR] Unexpected argument: $1"
            exit 1
            ;;
    esac
done

case "$ACTION" in
    create|list|delete) ;;
    "")
        echo "[ERROR] Required: --action create|list|delete"
        exit 1
        ;;
    *)
        echo "[ERROR] Unsupported --action: ${ACTION}"
        echo "[ERROR] Allowed: create, list, delete"
        exit 1
        ;;
esac

# ========================================================================= Methods

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

rand_chars() {
    local charset="$1"
    local len="$2"
    local out
    set +o pipefail
    out="$(tr -dc "$charset" </dev/urandom | head -c "$len")"
    set -o pipefail
    if [ "${#out}" -ne "$len" ]; then
        echo "[ERROR] Failed to generate ${len} random characters" >&2
        exit 1
    fi
    printf '%s' "$out"
}

generate_identifier() {
    case "$1" in
        p_alnum)   echo "p$(rand_chars 'a-z0-9' 12)" ;;
        u_hex)     echo "u$(rand_chars 'a-f0-9' 16)" ;;
        app_alnum) echo "app_$(rand_chars 'a-z0-9' 10)" ;;
        db_snake)  echo "db_$(rand_chars 'a-z0-9' 8)_$(rand_chars 'a-z0-9' 4)" ;;
        r_digit)   echo "r$(rand_chars 'a-z' 4)$(rand_chars '0-9' 8)" ;;
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
        alnum_sym28) rand_chars 'A-Za-z0-9!@#%^*_-' 28 ;;
        base58_32)   rand_chars '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz' 32 ;;
        *)
            echo "[ERROR] Internal: unknown password format $1" >&2
            exit 1
            ;;
    esac
}

sql_quote() {
    printf "%s" "$1" | sed "s/'/''/g"
}

url_encode() {
    if command -v python3 &>/dev/null; then
        python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1], safe=""))' "$1"
    else
        printf "%s" "$1"
    fi
}

psql_admin() {
    sudo -u postgres psql -p "$RHCTL_PG_PORT" -d postgres "$@"
}

ensure_ready() {
    if ! command -v psql &>/dev/null; then
        echo "[ERROR] psql not found. Run install.sh and configure.sh first."
        exit 1
    fi
    if ! command -v pg_isready &>/dev/null; then
        echo "[ERROR] pg_isready not found."
        exit 1
    fi
    if ! pg_isready -h 127.0.0.1 -p "$RHCTL_PG_PORT" -d postgres >/dev/null 2>&1; then
        echo "[ERROR] PostgreSQL is not accepting connections on 127.0.0.1:${RHCTL_PG_PORT}"
        echo "[ERROR] Run configure.sh first, or check --port."
        exit 1
    fi
}

# ========================================================================= Preflight

ensure_ready

# ========================================================================= Action: list

if [ "$ACTION" = "list" ]; then
    log "Listing databases and owners on port ${RHCTL_PG_PORT}"
    echo "============================================="
    printf "%-32s %s\n" "DATABASE" "OWNER"
    printf "%-32s %s\n" "--------------------------------" "--------------------"
    psql_admin -v ON_ERROR_STOP=1 -tAc \
        "SELECT d.datname || E'\t' || pg_catalog.pg_get_userbyid(d.datdba)
         FROM pg_catalog.pg_database d
         WHERE NOT d.datistemplate
         ORDER BY 1;" \
        | while IFS=$'\t' read -r db owner; do
            [ -z "$db" ] && continue
            printf "%-32s %s\n" "$db" "$owner"
        done
    echo "============================================="
    exit 0
fi

# ========================================================================= Action: delete

if [ "$ACTION" = "delete" ]; then
    if [ -z "$DELETE_DB_NAME" ]; then
        echo "[ERROR] --db-name is required for --action delete"
        exit 1
    fi
    if in_list "$DELETE_DB_NAME" "${PROTECTED_DBS[@]}"; then
        echo "[ERROR] Refusing to drop protected database: ${DELETE_DB_NAME}"
        exit 1
    fi

    exists="$(psql_admin -tAc "SELECT 1 FROM pg_database WHERE datname='$(sql_quote "$DELETE_DB_NAME")'" || true)"
    if [ "$exists" != "1" ]; then
        echo "[ERROR] Database not found: ${DELETE_DB_NAME}"
        exit 1
    fi

    OWNER="$(psql_admin -tAc "SELECT pg_catalog.pg_get_userbyid(datdba) FROM pg_database WHERE datname='$(sql_quote "$DELETE_DB_NAME")'" | tr -d '[:space:]')"
    ROLE_TO_DROP="${DELETE_USER_NAME:-$OWNER}"

    log "Dropping database '${DELETE_DB_NAME}' (owner=${OWNER})"
    psql_admin -v ON_ERROR_STOP=1 -c \
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='$(sql_quote "$DELETE_DB_NAME")' AND pid <> pg_backend_pid();" \
        >/dev/null || true
    psql_admin -v ON_ERROR_STOP=1 -c "DROP DATABASE \"${DELETE_DB_NAME}\";"
    log "Dropped database '${DELETE_DB_NAME}'"

    if [ -n "$ROLE_TO_DROP" ]; then
        if in_list "$ROLE_TO_DROP" "${PROTECTED_ROLES[@]}"; then
            warn "Skipping drop of protected role '${ROLE_TO_DROP}'"
        else
            role_exists="$(psql_admin -tAc "SELECT 1 FROM pg_roles WHERE rolname='$(sql_quote "$ROLE_TO_DROP")'" || true)"
            if [ "$role_exists" = "1" ]; then
                owns_other="$(psql_admin -tAc "SELECT count(*) FROM pg_database WHERE pg_catalog.pg_get_userbyid(datdba)='$(sql_quote "$ROLE_TO_DROP")'" | tr -d '[:space:]')"
                if [ "${owns_other:-0}" != "0" ]; then
                    warn "Role '${ROLE_TO_DROP}' still owns ${owns_other} database(s) — not dropped"
                else
                    log "Dropping role '${ROLE_TO_DROP}'"
                    psql_admin -v ON_ERROR_STOP=1 -c "DROP ROLE \"${ROLE_TO_DROP}\";"
                    log "Dropped role '${ROLE_TO_DROP}'"
                fi
            else
                warn "Role '${ROLE_TO_DROP}' not found — skipped"
            fi
        fi
    fi

    echo "============================================="
    echo "[INFO] Delete complete"
    echo "[INFO]   DB_NAME=${DELETE_DB_NAME}"
    echo "[INFO]   USER_NAME=${ROLE_TO_DROP}"
    echo "============================================="
    exit 0
fi

# ========================================================================= Action: create

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
    exit 1
fi
if ! in_list "$USER_NAME_FORMAT" "${NAME_FORMATS[@]}"; then
    echo "[ERROR] Unsupported --user-name-format: ${USER_NAME_FORMAT}"
    exit 1
fi
if ! in_list "$PASSWORD_FORMAT" "${PASSWORD_FORMATS[@]}"; then
    echo "[ERROR] Unsupported --password-format: ${PASSWORD_FORMAT}"
    exit 1
fi

DB_CONNECT_HOST="${RHCTL_PG_HOST:-127.0.0.1}"
DB_NAME="$(generate_identifier "$DB_NAME_FORMAT")"
DB_USER="$(generate_identifier "$USER_NAME_FORMAT")"
DB_PASSWORD="$(generate_password "$PASSWORD_FORMAT")"

tries=0
while [ "$tries" -lt 8 ]; do
    role_exists="$(psql_admin -tAc "SELECT 1 FROM pg_roles WHERE rolname='${DB_USER}'" || true)"
    db_exists="$(psql_admin -tAc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'" || true)"
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

# ========================================================================= Persist state file

sudo mkdir -p "$(dirname "$STATE_FILE")"
sudo tee "$STATE_FILE" >/dev/null <<EOF
DB_NAME=${DB_NAME}
DB_USER=${DB_USER}
DB_PASSWORD=${DB_PASSWORD}
DB_HOST=${DB_CONNECT_HOST}
DB_PORT=${RHCTL_PG_PORT}
DB_CREDENTIAL_PROFILE=${CREDENTIAL_PROFILE}
DB_NAME_FORMAT=${DB_NAME_FORMAT}
DB_USER_NAME_FORMAT=${USER_NAME_FORMAT}
DB_PASSWORD_FORMAT=${PASSWORD_FORMAT}
EOF
sudo chmod 600 "$STATE_FILE"
sudo chown postgres:postgres "$STATE_FILE" 2>/dev/null || true
log "Generated new credentials → ${STATE_FILE}"
log "Formats: profile=${CREDENTIAL_PROFILE} db=${DB_NAME_FORMAT} user=${USER_NAME_FORMAT} password=${PASSWORD_FORMAT}"

# ========================================================================= Create role / database

DB_PASSWORD_SQL="$(sql_quote "$DB_PASSWORD")"
DB_PASSWORD_URL="$(url_encode "$DB_PASSWORD")"

psql_admin -v ON_ERROR_STOP=1 -c "CREATE USER ${DB_USER} WITH PASSWORD '${DB_PASSWORD_SQL}';"
log "Created role '${DB_USER}'"

psql_admin -v ON_ERROR_STOP=1 -c "CREATE DATABASE ${DB_NAME} OWNER ${DB_USER};"
log "Created database '${DB_NAME}' owned by '${DB_USER}'"

# ========================================================================= Final output

echo "============================================="
echo "[INFO] PostgreSQL database created"
echo "[INFO]   DB_NAME=${DB_NAME}"
echo "[INFO]   DB_USER=${DB_USER}"
echo "[INFO]   DB_PASSWORD=${DB_PASSWORD}"
echo "[INFO]   DB_HOST=${DB_CONNECT_HOST}"
echo "[INFO]   DB_PORT=${RHCTL_PG_PORT}"
echo "[INFO]   CREDENTIAL_PROFILE=${CREDENTIAL_PROFILE}"
echo "[INFO]   DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD_URL}@${DB_CONNECT_HOST}:${RHCTL_PG_PORT}/${DB_NAME}"
echo "[INFO] Test:"
echo "[INFO]   PGPASSWORD='${DB_PASSWORD}' psql -U ${DB_USER} -d ${DB_NAME} -h ${DB_CONNECT_HOST} -p ${RHCTL_PG_PORT}"
echo "============================================="
