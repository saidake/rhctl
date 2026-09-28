#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Configure installed PostgreSQL (listen, port, pg_hba, firewall)
# and ensure the service accepts connections.
#
# Does not create databases or roles — use db.sh --action create.
#
# Remote pg_hba access requires --allowed-ips; without it, only local
# 127.0.0.1 / ::1 rules remain.
#
# If exists, overwrite — safe to re-run.
#
# Usage:
#   ./configure.sh --port 5432
#   ./configure.sh \
#     --port 5432 \
#     --auth-method md5 \
#     --allowed-ips 192.168.1.100,10.0.0.5
#
# Required Parameters:
#   (none)
#
# Optional Parameters:
#   --port <port>
#       Listen port (default: `5432`).
#         Example port values: `5432`, `5433`, `48985`
#   --auth-method <method>
#       pg_hba auth method (default: `md5`).
#         Example method values: `md5`, `scram-sha-256`, `password`
#   --allowed-ips <ip>[,<ip>...]
#       Remote pg_hba allow-list; re-runs overwrite prior remote host-all rules.
#       When omitted, remote rules are cleared (local `127.0.0.1` / `::1` only).
#         Example ip values: `192.168.1.100`, `10.0.0.5`, `192.168.1.0/24`,
#           `0.0.0.0/0`, `::/0`
#
# Override Parameters:
#   RHCTL_PG_PORT=<port>
#       Same as `--port`.
#   RHCTL_PG_AUTH_METHOD=<method>
#       Same as `--auth-method`.
#   RHCTL_PG_ALLOW_IPS=<ip>[,<ip>...]
#       Same as `--allowed-ips`.
#
# Since : 1.0.3
# Date  : Sep 27, 2026
# ************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

RHCTL_PG_PORT="${RHCTL_PG_PORT:-5432}"
RHCTL_PG_AUTH_METHOD="${RHCTL_PG_AUTH_METHOD:-md5}"
NEED_RESTART=false
IPS=()

while [ "$#" -gt 0 ]; do
    case "$1" in
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
        -h|--help)
            sed -n '2,50p' "$0"
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

# ========================================================================= Methods

log() { echo "[INFO] $*"; }
warn() { echo "[WARN] $*"; }

set_conf_kv() {
    local key="$1"
    local value="$2"
    local file="$3"
    local expected="${key} = ${value}"
    local current

    current=$(grep -E "^[[:space:]]*${key}[[:space:]]*=" "$file" | head -1 || true)
    if [ -n "$current" ]; then
        local normalized
        normalized=$(echo "$current" \
            | sed 's/^[[:space:]]*//;s/[[:space:]]*=[[:space:]]*/ = /;s/[[:space:]]\+/ /g')
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

# ========================================================================= PostgreSQL discovery

if ! command -v psql &>/dev/null; then
    echo "[ERROR] psql not found. Run install.sh first."
    exit 1
fi

if ! command -v pg_lsclusters &>/dev/null; then
    echo "[ERROR] pg_lsclusters not found."
    echo "[ERROR] This script requires the Debian/Ubuntu postgresql-common package."
    exit 1
fi

CLUSTER_INFO="$(pg_lsclusters -h 2>/dev/null | awk '$2 == "main" { print; exit }')"

if [ -z "$CLUSTER_INFO" ]; then
    echo "[ERROR] PostgreSQL 'main' cluster not found."
    echo "[ERROR] Available clusters:"
    pg_lsclusters || true
    exit 1
fi

PG_VERSION="$(echo "$CLUSTER_INFO" | awk '{print $1}')"
PG_CLUSTER="$(echo "$CLUSTER_INFO" | awk '{print $2}')"
CLUSTER_PORT="$(echo "$CLUSTER_INFO" | awk '{print $3}')"
CLUSTER_STATUS="$(echo "$CLUSTER_INFO" | awk '{print $4}')"

CONF_DIR="/etc/postgresql/${PG_VERSION}/${PG_CLUSTER}"
CONF_FILE="${CONF_DIR}/postgresql.conf"
HBA_FILE="${CONF_DIR}/pg_hba.conf"

if [ ! -f "$CONF_FILE" ] || [ ! -f "$HBA_FILE" ]; then
    echo "[ERROR] Expected PostgreSQL config files were not found:"
    echo "[ERROR]   ${CONF_FILE}"
    echo "[ERROR]   ${HBA_FILE}"
    exit 1
fi

log "Detected PostgreSQL cluster: ${PG_VERSION}/${PG_CLUSTER}"
log "Current cluster port: ${CLUSTER_PORT}"
log "Current cluster status: ${CLUSTER_STATUS}"

# ========================================================================= listen / port

if set_conf_kv "listen_addresses" "'*'" "$CONF_FILE"; then
    NEED_RESTART=true
fi

if set_conf_kv "port" "${RHCTL_PG_PORT}" "$CONF_FILE"; then
    NEED_RESTART=true
fi

AUTH="$RHCTL_PG_AUTH_METHOD"

# ========================================================================= remote pg_hba

DESIRED_CIDRS=()
ALLOW_IPS_EXPLICIT=false

if [ "${#IPS[@]}" -gt 0 ]; then
    ALLOW_IPS_EXPLICIT=true
    for raw in "${IPS[@]}"; do
        ip="$(echo "$raw" | xargs)"
        [ -z "$ip" ] && continue
        DESIRED_CIDRS+=("$(normalize_cidr "$ip")")
    done
fi

if [ "$ALLOW_IPS_EXPLICIT" = true ] && [ "${#DESIRED_CIDRS[@]}" -eq 0 ]; then
    echo "[ERROR] --allowed-ips produced an empty allow-list"
    exit 1
fi

if [ "${#DESIRED_CIDRS[@]}" -eq 0 ]; then
    log "No --allowed-ips — clearing remote pg_hba rules (local only)"
fi

if sync_remote_hba_allowlist "${DESIRED_CIDRS[@]}"; then
    NEED_RESTART=true
fi

# ========================================================================= firewall

configure_firewall "$RHCTL_PG_PORT"

# ========================================================================= Start / restart PostgreSQL

if [ "$NEED_RESTART" = true ]; then
    log "Restarting PostgreSQL to apply configuration"
    if ! sudo systemctl restart postgresql; then
        echo "[ERROR] Failed to restart PostgreSQL"
        sudo systemctl status postgresql --no-pager || true
        exit 1
    fi
else
    if ! systemctl is-active --quiet postgresql; then
        warn "PostgreSQL service is not active — attempting start"
        if ! sudo systemctl start postgresql; then
            echo "[ERROR] Failed to start PostgreSQL"
            sudo systemctl status postgresql --no-pager || true
            exit 1
        fi
    else
        log "PostgreSQL service is already active"
    fi
fi

# ========================================================================= Wait until PostgreSQL accepts connections

if ! command -v pg_isready &>/dev/null; then
    echo "[ERROR] pg_isready not found."
    exit 1
fi

log "Waiting for PostgreSQL on 127.0.0.1:${RHCTL_PG_PORT}"

READY=false
for _ in $(seq 1 30); do
    if pg_isready \
        -h 127.0.0.1 \
        -p "$RHCTL_PG_PORT" \
        -d postgres \
        >/dev/null 2>&1; then
        READY=true
        break
    fi
    sleep 1
done

if [ "$READY" != true ]; then
    echo "[ERROR] PostgreSQL is not accepting connections on"
    echo "[ERROR]   127.0.0.1:${RHCTL_PG_PORT}"
    echo
    echo "[ERROR] Cluster status:"
    pg_lsclusters || true
    echo
    echo "[ERROR] Listening sockets:"
    sudo ss -lntp | grep -E "(:${RHCTL_PG_PORT}[[:space:]]|postgres)" || true
    echo
    echo "[ERROR] PostgreSQL service status:"
    sudo systemctl status postgresql --no-pager || true
    exit 1
fi

log "PostgreSQL is accepting connections on 127.0.0.1:${RHCTL_PG_PORT}"

# ========================================================================= Verify runtime configuration

RUNTIME_PORT="$(
    sudo -u postgres psql \
        -p "$RHCTL_PG_PORT" \
        -d postgres \
        -tAc "SHOW port;" \
        | tr -d '[:space:]'
)"

RUNTIME_LISTEN="$(
    sudo -u postgres psql \
        -p "$RHCTL_PG_PORT" \
        -d postgres \
        -tAc "SHOW listen_addresses;" \
        | tr -d '[:space:]'
)"

if [ "$RUNTIME_PORT" != "$RHCTL_PG_PORT" ]; then
    echo "[ERROR] PostgreSQL runtime port mismatch."
    echo "[ERROR] Expected: ${RHCTL_PG_PORT}"
    echo "[ERROR] Actual:   ${RUNTIME_PORT}"
    exit 1
fi

log "Runtime listen_addresses=${RUNTIME_LISTEN}"
log "Runtime port=${RUNTIME_PORT}"

# ========================================================================= Final output

echo "============================================="
echo "[INFO] PostgreSQL server configured"
echo "[INFO]   LISTEN=* PORT=${RHCTL_PG_PORT} AUTH=${AUTH}"
if [ "${#DESIRED_CIDRS[@]}" -gt 0 ]; then
    echo "[INFO]   ALLOW_IPS=${DESIRED_CIDRS[*]}"
else
    echo "[INFO]   ALLOW_IPS=(none — local 127.0.0.1 / ::1 only)"
fi
echo "[INFO] Next: create a database with"
echo "[INFO]   ./db.sh --action create --port ${RHCTL_PG_PORT}"
echo "============================================="
