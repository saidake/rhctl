#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Execute one or more SQL files against a PostgreSQL database.
#
# Connection resolution (first match wins):
#   1) DATABASE_URL
#   2) state file from configure.sh
#   3) PG* environment variables
#
# Usage:
#   ./execute-sql.sh \
#     --dir /opt/pidifa/ddl \
#     --continue
#
# Required Parameters:
#   --dir <directory>
#       Default, run all `*.sql` files in the directory (sorted); may repeat.
#         Example directory values: `./sql`, `/opt/pidifa/ddl`
#
# Optional Parameters:
#   --continue
#       Do not stop on the first SQL error (default: stop).
#
# Override Parameters:
#   DATABASE_URL=<url>
#       Postgres connection URL (preferred).
#         Example url values: `postgres://user:pass@127.0.0.1:5432/mydb`
#   RHCTL_PG_STATE_FILE=<path>
#       Latest credentials state file written by `configure.sh`.
#   PGHOST / PGPORT / PGUSER / PGPASSWORD / PGDATABASE
#       Libpq connection overrides when `DATABASE_URL` is unset.
#
# Since : 1.0.1
# Date  : Sep 26, 2026
# ************************************************************************************

set -euo pipefail

log() { echo "[INFO] $*"; }
err() { echo "[ERROR] $*" >&2; }

CONTINUE_ON_ERROR=false
FILES=()
DIRS=()

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dir)
            DIRS+=("$2")
            shift 2
            ;;
        --continue)
            CONTINUE_ON_ERROR=true
            shift
            ;;
        -h|--help)
            sed -n '2,45p' "$0"
            exit 0
            ;;
        --*)
            err "Unknown option: $1"
            exit 1
            ;;
        *)
            err "Unexpected argument: $1 (use --dir <directory>)"
            exit 1
            ;;
    esac
done

if [ "${#DIRS[@]}" -eq 0 ]; then
    err "Required: --dir <directory>"
    exit 1
fi

for d in "${DIRS[@]}"; do
    if [ ! -d "$d" ]; then
        err "Not a directory: $d"
        exit 1
    fi
    while IFS= read -r f; do
        FILES+=("$f")
    done < <(find "$d" -maxdepth 1 -type f -name '*.sql' | sort)
done

if [ "${#FILES[@]}" -eq 0 ]; then
    err "No *.sql files found under: ${DIRS[*]}"
    exit 1
fi

STATE_FILE="${RHCTL_PG_STATE_FILE:-/var/lib/postgresql/.rhctl-pg-test-credentials}"

build_psql_args() {
    if [ -n "${DATABASE_URL:-}" ]; then
        PSQL_ARGS=(-d "$DATABASE_URL")
        return 0
    fi
    if [ -f "$STATE_FILE" ]; then
        # shellcheck disable=SC1090
        source "$STATE_FILE"
        export PGPASSWORD="${DB_PASSWORD}"
        PSQL_ARGS=(-h "${PGHOST:-127.0.0.1}" -p "${DB_PORT:-${PGPORT:-5432}}" -U "${DB_USER}" -d "${DB_NAME}")
        return 0
    fi
    if [ -n "${PGDATABASE:-}" ] && [ -n "${PGUSER:-}" ]; then
        PSQL_ARGS=(-h "${PGHOST:-127.0.0.1}" -p "${PGPORT:-5432}" -U "$PGUSER" -d "$PGDATABASE")
        return 0
    fi
    err "Set DATABASE_URL, or run configure.sh first (state file), or set PG* vars"
    exit 1
}

build_psql_args

ON_ERROR_STOP=1
if [ "$CONTINUE_ON_ERROR" = true ]; then
    ON_ERROR_STOP=0
fi

failures=0
for sql in "${FILES[@]}"; do
    if [ ! -f "$sql" ]; then
        err "Missing file: $sql"
        failures=$((failures + 1))
        [ "$CONTINUE_ON_ERROR" = true ] || exit 1
        continue
    fi
    log "Executing ${sql}"
    if ! psql "${PSQL_ARGS[@]}" -v ON_ERROR_STOP="${ON_ERROR_STOP}" -f "$sql"; then
        err "Failed: ${sql}"
        failures=$((failures + 1))
        [ "$CONTINUE_ON_ERROR" = true ] || exit 1
    else
        log "OK: ${sql}"
    fi
done

if [ "$failures" -gt 0 ]; then
    err "${failures} file(s) failed"
    exit 1
fi

log "All SQL files applied successfully"
