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
#   clear  — drop all non-system databases and non-system roles
#
# Usage:
#   ./db.sh --action create --port 5432
#   ./db.sh --action list --port 5432
#   ./db.sh --action delete --db-name mydb --user-name myuser --port 5432
#   ./db.sh --action clear --port 5432
#
# Parameters (common):
#   --action <action>
#       Default, operation to run.
#         Example action values: `create`, `list`, `delete`, `clear`
#   --port <port>
#       PostgreSQL port (default: `5432`).
#         Example port values: `5432`, `5433`
#
# Parameters (create):
#   --max-length
#       Use maximum lengths for DB name, user name, and password
#       (identifiers: 63; password: 72). Default: random length in range.
#
# Parameters (list):
#   (none beyond common)
#
# Parameters (delete):
#   --db-name <name>
#       Database name to drop (required for delete).
#         Example name values: `xk7m2npq9r4s…`
#   --user-name <name>
#       Role to drop (default: database owner).
#         Example name values: `ab3c8d1ef…`
#
# Parameters (clear):
#   (none beyond common)
#       Keeps system DBs (`postgres`, `template0`, `template1`) and the
#       `postgres` role / `pg_*` system roles.
#
# Override Parameters:
#   RHCTL_PG_PORT=<port>
#       Same as `--port`.
#   RHCTL_PG_STATE_FILE=<path>
#       Credentials state file written by `create` (used by execute-sql.sh).
#         Example path values: `/var/lib/postgresql/.rhctl-pg-test-credentials`
#   RHCTL_PG_MAX_LENGTH=1
#       Same as `--max-length` when set to a non-empty value.
#
# Since : 1.0.3
# Date  : Sep 28, 2026
# ************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

RHCTL_PG_PORT="${RHCTL_PG_PORT:-5432}"
STATE_FILE="${RHCTL_PG_STATE_FILE:-/var/lib/postgresql/.rhctl-pg-test-credentials}"
MAX_LENGTH=false
if [ -n "${RHCTL_PG_MAX_LENGTH:-}" ]; then
    MAX_LENGTH=true
fi
ACTION=""
DELETE_DB_NAME=""
DELETE_USER_NAME=""

PROTECTED_DBS=(postgres template0 template1)
PROTECTED_ROLES=(postgres)

# Unquoted PG identifier: [a-z][a-z0-9_]*, max NAMEDATALEN-1 (63).
IDENT_MIN_LEN=24
IDENT_MAX_LEN=63
# Quoted password: broad printable set; length within a strong range.
PASSWORD_MIN_LEN=32
PASSWORD_MAX_LEN=72
# '-' last so `tr` treats it as literal, not a range.
PASSWORD_CHARSET='A-Za-z0-9!@#%^&*_+=[]{}|;:,.<>?/~-'

# Placeholder in printed DATABASE_URL / state file (replace with the client-facing host).
DB_HOST_PLACEHOLDER='<host>'

while [ "$#" -gt 0 ]; do
    case "$1" in
        --action)
            ACTION="$2"
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
        --max-length)
            MAX_LENGTH=true
            shift
            ;;
        -h|--help)
            sed -n '2,65p' "$0"
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
    create|list|delete|clear) ;;
    "")
        echo "[ERROR] Required: --action create|list|delete|clear"
        exit 1
        ;;
    *)
        echo "[ERROR] Unsupported --action: ${ACTION}"
        echo "[ERROR] Allowed: create, list, delete, clear"
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

rand_int() {
    local min="$1"
    local max="$2"
    echo $((min + RANDOM % (max - min + 1)))
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

# Random-length unquoted identifier using the full allowed charset.
generate_identifier() {
    local len first rest
    if [ "$MAX_LENGTH" = true ]; then
        len="$IDENT_MAX_LEN"
    else
        len="$(rand_int "$IDENT_MIN_LEN" "$IDENT_MAX_LEN")"
    fi
    first="$(rand_chars 'a-z' 1)"
    rest="$(rand_chars 'a-z0-9_' "$((len - 1))")"
    printf '%s%s' "$first" "$rest"
}

# Random-length password using a dense printable charset (SQL-quoted / URL-encoded later).
generate_password() {
    local len
    if [ "$MAX_LENGTH" = true ]; then
        len="$PASSWORD_MAX_LEN"
    else
        len="$(rand_int "$PASSWORD_MIN_LEN" "$PASSWORD_MAX_LEN")"
    fi
    rand_chars "$PASSWORD_CHARSET" "$len"
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

# ========================================================================= Action: clear

if [ "$ACTION" = "clear" ]; then
    log "Clearing non-system databases and roles on port ${RHCTL_PG_PORT}"

    mapfile -t DROP_DBS < <(
        psql_admin -tAc \
            "SELECT datname FROM pg_database
             WHERE NOT datistemplate
               AND datname <> 'postgres'
             ORDER BY 1;" \
            | sed '/^$/d'
    )

    DROPPED_DBS=0
    for db in "${DROP_DBS[@]+"${DROP_DBS[@]}"}"; do
        [ -z "$db" ] && continue
        if in_list "$db" "${PROTECTED_DBS[@]}"; then
            warn "Skipping protected database '${db}'"
            continue
        fi
        log "Dropping database '${db}'"
        psql_admin -v ON_ERROR_STOP=1 -c \
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='$(sql_quote "$db")' AND pid <> pg_backend_pid();" \
            >/dev/null || true
        psql_admin -v ON_ERROR_STOP=1 -c "DROP DATABASE \"${db}\";"
        log "Dropped database '${db}'"
        DROPPED_DBS=$((DROPPED_DBS + 1))
    done

    mapfile -t DROP_ROLES < <(
        psql_admin -tAc \
            "SELECT rolname FROM pg_roles
             WHERE rolname <> 'postgres'
               AND rolname NOT LIKE 'pg\_%' ESCAPE '\'
             ORDER BY 1;" \
            | sed '/^$/d'
    )

    DROPPED_ROLES=0
    for role in "${DROP_ROLES[@]+"${DROP_ROLES[@]}"}"; do
        [ -z "$role" ] && continue
        if in_list "$role" "${PROTECTED_ROLES[@]}"; then
            warn "Skipping protected role '${role}'"
            continue
        fi
        owns="$(psql_admin -tAc "SELECT count(*) FROM pg_database WHERE pg_catalog.pg_get_userbyid(datdba)='$(sql_quote "$role")'" | tr -d '[:space:]')"
        if [ "${owns:-0}" != "0" ]; then
            warn "Role '${role}' still owns ${owns} database(s) — not dropped"
            continue
        fi
        log "Dropping role '${role}'"
        psql_admin -v ON_ERROR_STOP=1 -c "DROP ROLE \"${role}\";"
        log "Dropped role '${role}'"
        DROPPED_ROLES=$((DROPPED_ROLES + 1))
    done

    echo "============================================="
    echo "[INFO] Clear complete"
    echo "[INFO]   Dropped databases: ${DROPPED_DBS}"
    echo "[INFO]   Dropped roles:     ${DROPPED_ROLES}"
    echo "[INFO]   Kept: postgres DB + template* + postgres/pg_* roles"
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

DB_CONNECT_HOST="$DB_HOST_PLACEHOLDER"
DB_NAME="$(generate_identifier)"
DB_USER="$(generate_identifier)"
DB_PASSWORD="$(generate_password)"

tries=0
while [ "$tries" -lt 8 ]; do
    role_exists="$(psql_admin -tAc "SELECT 1 FROM pg_roles WHERE rolname='${DB_USER}'" || true)"
    db_exists="$(psql_admin -tAc "SELECT 1 FROM pg_database WHERE datname='${DB_NAME}'" || true)"
    if [ "$role_exists" != "1" ] && [ "$db_exists" != "1" ]; then
        break
    fi
    [ "$role_exists" = "1" ] && DB_USER="$(generate_identifier)"
    [ "$db_exists" = "1" ] && DB_NAME="$(generate_identifier)"
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
EOF
sudo chmod 600 "$STATE_FILE"
sudo chown postgres:postgres "$STATE_FILE" 2>/dev/null || true
log "Generated new credentials → ${STATE_FILE}"
log "Lengths: db=${#DB_NAME} user=${#DB_USER} password=${#DB_PASSWORD}"

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
echo "[INFO]   DATABASE_URL=postgres://${DB_USER}:${DB_PASSWORD_URL}@${DB_CONNECT_HOST}:${RHCTL_PG_PORT}/${DB_NAME}"
echo "[INFO] Test:"
echo "[INFO]   PGPASSWORD='${DB_PASSWORD}' psql -U ${DB_USER} -d ${DB_NAME} -h ${DB_CONNECT_HOST} -p ${RHCTL_PG_PORT}"
echo "============================================="
