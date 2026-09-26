#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Set PostgreSQL remote access to an exact allow-list of client addresses.
#
# Replaces existing remote `host all all …` rules (keeps localhost 127.0.0.1/32
# and ::1/128). Re-runs overwrite the previous allow-list.
#
# Idempotent — safe to re-run.
#
# Usage:
#   ./limit-remote-ips.sh \
#     --allowed-ips 192.168.1.100,10.0.0.5,192.168.1.0/24 \
#     --auth-method md5
#   ./limit-remote-ips.sh \
#     --allowed-ips 0.0.0.0/0,::/0 \
#     --auth-method md5
#
# Required Parameters:
#   --allowed-ips <ip>[,<ip>...]
#       Desired final allow-list (overwrites prior remote host-all rules).
#       Use `0.0.0.0/0,::/0` to allow all IPv4 + IPv6 again.
#         Example ip values: `192.168.1.100`, `10.0.0.5`, `192.168.1.0/24`,
#           `0.0.0.0/0`, `::/0`
#
# Optional Parameters:
#   --auth-method <method>
#       pg_hba auth method (default: `md5`).
#         Example method values: `md5`, `scram-sha-256`, `password`
#
# Override Parameters:
#   RHCTL_PG_ALLOW_IPS=<ip>[,<ip>...]
#       Same as `--allowed-ips`.
#   RHCTL_PG_AUTH_METHOD=<method>
#       Same as `--auth-method`.
#
# Since : 1.0.1
# Date  : Sep 26, 2026
# ************************************************************************************

set -euo pipefail

log() { echo "[INFO] $*"; }

RHCTL_PG_AUTH_METHOD="${RHCTL_PG_AUTH_METHOD:-md5}"
IPS=()

while [ "$#" -gt 0 ]; do
    case "$1" in
        --allowed-ips)
            IFS=',' read -r -a _parsed <<<"$2"
            IPS+=("${_parsed[@]}")
            shift 2
            ;;
        --auth-method)
            RHCTL_PG_AUTH_METHOD="$2"
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

if [ "${#IPS[@]}" -eq 0 ]; then
    echo "[ERROR] Required: --allowed-ips IP[,IP...] or positional IPs (or RHCTL_PG_ALLOW_IPS)"
    exit 1
fi

case "$RHCTL_PG_AUTH_METHOD" in
    md5|scram-sha-256|password|trust|reject) ;;
    *)
        echo "[ERROR] Unsupported --auth-method: ${RHCTL_PG_AUTH_METHOD}"
        echo "[ERROR] Allowed: md5, scram-sha-256, password, trust, reject"
        exit 1
        ;;
esac

AUTH="$RHCTL_PG_AUTH_METHOD"

if ! command -v psql &>/dev/null; then
    echo "[ERROR] psql not found. Run install.sh first."
    exit 1
fi

PG_VERSION=$(psql --version 2>&1 | grep -oP '(?<=psql \(PostgreSQL\) )[\d.]+' | head -1 | cut -d. -f1)
HBA_FILE="/etc/postgresql/${PG_VERSION}/main/pg_hba.conf"

if [ ! -f "$HBA_FILE" ]; then
    echo "[ERROR] Missing ${HBA_FILE}"
    exit 1
fi

normalize_cidr() {
    local ip="$1"
    if [[ "$ip" != */* ]]; then
        echo "${ip}/32"
    else
        echo "$ip"
    fi
}

# Snapshot remote host-all lines (exclude localhost).
remote_hba_snapshot() {
    sudo grep -E '^host[[:space:]]+all[[:space:]]+all[[:space:]]+' "$HBA_FILE" 2>/dev/null \
        | grep -Ev '[[:space:]](127\.0\.0\.1/32|::1/128)[[:space:]]' || true
}

# Drop all remote host-all rules; keep localhost.
remove_remote_hba_rules() {
    sudo sed -i -E \
        '/^host[[:space:]]+all[[:space:]]+all[[:space:]]+(127\.0\.0\.1\/32|::1\/128)[[:space:]]+/b
         /^host[[:space:]]+all[[:space:]]+all[[:space:]]+/d' \
        "$HBA_FILE"
}

DESIRED_CIDRS=()
for raw in "${IPS[@]}"; do
    ip="$(echo "$raw" | xargs)"
    [ -z "$ip" ] && continue
    DESIRED_CIDRS+=("$(normalize_cidr "$ip")")
done

if [ "${#DESIRED_CIDRS[@]}" -eq 0 ]; then
    echo "[ERROR] --allowed-ips produced an empty allow-list"
    exit 1
fi

BEFORE="$(remote_hba_snapshot)"
remove_remote_hba_rules

for cidr in "${DESIRED_CIDRS[@]}"; do
    line="host    all             all             ${cidr}               ${AUTH}"
    echo "$line" | sudo tee -a "$HBA_FILE" >/dev/null
    log "Allowed ${cidr} with auth-method ${AUTH}"
done

AFTER="$(remote_hba_snapshot)"

if [ "$BEFORE" != "$AFTER" ]; then
    log "Reloading PostgreSQL"
    sudo systemctl reload postgresql || sudo systemctl restart postgresql
else
    log "No pg_hba.conf changes"
fi

log "Current host lines:"
sudo grep -E '^host[[:space:]]' "$HBA_FILE" || true
