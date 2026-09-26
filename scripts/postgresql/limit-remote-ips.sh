#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Restrict PostgreSQL remote access to an allow-list of client IPs.
#
# Removes open "0.0.0.0/0" / "::/0" host-all rules added by init.sh (any auth method)
# and ensures one `host all all <ip>/32 <auth-method>` line per IP.
#
# Idempotent — safe to re-run.
#
# Usage:
#   ./limit-remote-ips.sh \
#     --ips 192.168.1.100,10.0.0.5,192.168.1.0/24 \
#     --auth-method md5
#
# Required Parameters:
#   --ips <ip>[,<ip>...]
#       Allow-list of client addresses (or pass IPs as positional args).
#         Example ip values: `192.168.1.100`, `10.0.0.5`, `192.168.1.0/24`
#
# Optional Parameters:
#   --auth-method <method>
#       pg_hba auth method (default: `md5`).
#         Example method values: `md5`, `scram-sha-256`, `password`
#
# Override Parameters:
#   RHCTL_PG_ALLOW_IPS=<ip>[,<ip>...]
#       Same as `--ips`.
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
        --ips)
            IFS=',' read -r -a _parsed <<<"$2"
            IPS+=("${_parsed[@]}")
            shift 2
            ;;
        --auth-method)
            RHCTL_PG_AUTH_METHOD="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '2,40p' "$0"
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
    echo "[ERROR] Required: --ips IP[,IP...] or positional IPs (or RHCTL_PG_ALLOW_IPS)"
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

CHANGED=false

# Drop wide-open rules regardless of auth method suffix.
if sudo grep -qE '^host[[:space:]]+all[[:space:]]+all[[:space:]]+0\.0\.0\.0/0[[:space:]]+' "$HBA_FILE"; then
    sudo sed -i -E '/^host[[:space:]]+all[[:space:]]+all[[:space:]]+0\.0\.0\.0\/0[[:space:]]+/d' "$HBA_FILE"
    log "Removed host all all 0.0.0.0/0 rules"
    CHANGED=true
fi
if sudo grep -qE '^host[[:space:]]+all[[:space:]]+all[[:space:]]+::/0[[:space:]]+' "$HBA_FILE"; then
    sudo sed -i -E '/^host[[:space:]]+all[[:space:]]+all[[:space:]]+::\/0[[:space:]]+/d' "$HBA_FILE"
    log "Removed host all all ::/0 rules"
    CHANGED=true
fi

for raw in "${IPS[@]}"; do
    ip="$(echo "$raw" | xargs)"
    [ -z "$ip" ] && continue
    if [[ "$ip" != */* ]]; then
        cidr="${ip}/32"
    else
        cidr="$ip"
    fi
    line="host    all             all             ${cidr}               ${AUTH}"
    escaped_cidr="${cidr//\//\\/}"
    if sudo grep -qE "^host[[:space:]]+all[[:space:]]+all[[:space:]]+${escaped_cidr}[[:space:]]+${AUTH}" "$HBA_FILE"; then
        log "Already allowed: ${cidr} (${AUTH})"
        continue
    fi
    echo "$line" | sudo tee -a "$HBA_FILE" >/dev/null
    log "Allowed ${cidr} with auth-method ${AUTH}"
    CHANGED=true
done

if [ "$CHANGED" = true ]; then
    log "Reloading PostgreSQL"
    sudo systemctl reload postgresql || sudo systemctl restart postgresql
else
    log "No pg_hba.conf changes"
fi

log "Current host lines:"
sudo grep -E '^host[[:space:]]' "$HBA_FILE" || true
