#!/bin/bash

# ****************************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Configures UFW to deny all incoming traffic except the specified ports.
#
# If exists, overwrite — safe to re-run.
#
# Usage:
#   ./configure.sh \
#     --port 2222 \
#     --port 80 \
#     --port 443
#
# Required Parameters:
#   --port <port>
#       Port to allow for incoming traffic. Can be specified multiple times.
#         Example port values: `22`, `80`, `443`, `2222`
#
# Optional Parameters:
#   (none)
#
# Override Parameters:
#   RHCTL_PORTS=<ports>
#       Comma-separated ports to allow for incoming traffic.
#         Example ports values: `22,80,443`, `80,443,2222`
#
# Since : 1.0.0
# Date  : Sep 28, 2026
# ****************************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

PORTS=()

if [[ -n "${RHCTL_PORTS:-}" ]]; then
    IFS=',' read -ra ENV_PORTS <<< "${RHCTL_PORTS}"

    for PORT in "${ENV_PORTS[@]}"; do
        PORTS+=("${PORT}")
    done
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)
            [[ $# -ge 2 ]] || {
                echo "Error: --port requires a value." >&2
                exit 1
            }

            PORTS+=("$2")
            shift 2
            ;;

        *)
            echo "Error: Unknown option $1" >&2
            exit 1
            ;;
    esac
done

if [[ ${#PORTS[@]} -eq 0 ]]; then
    echo "Error: At least one --port is required." >&2
    exit 1
fi

# ========================================================================= Constants

# ========================================================================= Methods

die() {
    echo "Error: $*" >&2
    exit 1
}

validate_port() {
    local PORT="$1"

    if ! [[ "${PORT}" =~ ^[0-9]+$ ]] || (( PORT < 1 || PORT > 65535 )); then
        die "Invalid port: ${PORT}"
    fi
}

# ========================================================================= Validate parameters

for PORT in "${PORTS[@]}"; do
    validate_port "${PORT}"
done

# Remove duplicate ports while preserving order.
UNIQUE_PORTS=()

for PORT in "${PORTS[@]}"; do
    DUPLICATE=false

    for EXISTING_PORT in "${UNIQUE_PORTS[@]}"; do
        if [[ "${PORT}" == "${EXISTING_PORT}" ]]; then
            DUPLICATE=true
            break
        fi
    done

    if [[ "${DUPLICATE}" == false ]]; then
        UNIQUE_PORTS+=("${PORT}")
    fi
done

PORTS=("${UNIQUE_PORTS[@]}")

# ========================================================================= Check UFW

if ! command -v ufw >/dev/null 2>&1; then
    die "ufw is not installed."
fi

# ========================================================================= Reset firewall rules

ufw --force reset

# ========================================================================= Configure default policy

ufw default deny incoming
ufw default allow outgoing

# ========================================================================= Allow ports

for PORT in "${PORTS[@]}"; do
    ufw allow "${PORT}/tcp"
done

# ========================================================================= Start / reload UFW

if ufw status | grep -q '^Status: inactive'; then
    ufw --force enable
else
    ufw reload
fi

# ========================================================================= Validate

UFW_STATUS="$(ufw status)"

if ! printf '%s\n' "${UFW_STATUS}" | grep -q '^Status: active'; then
    die "UFW is not active."
fi

for PORT in "${PORTS[@]}"; do
    if ! printf '%s\n' "${UFW_STATUS}" | grep -qE "^${PORT}/tcp[[:space:]]+ALLOW"; then
        die "Port ${PORT}/tcp is not allowed by UFW."
    fi
done

# ========================================================================= Final output

echo ""
echo " UFW Configuration Complete"
echo " Allowed Ports:"

for PORT in "${PORTS[@]}"; do
    echo "   - ${PORT}/tcp"
done

echo " Incoming      : DENY"
echo " Outgoing      : ALLOW"
echo ""
echo " UFW Status:"
ufw status