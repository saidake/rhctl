#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Configure an installed NATS JetStream server for remote client access.
#
# Ensures listen host/port in nats.conf, opens the firewall port (ufw when
# present), restarts the service when config changes, and prints a connection URL.
#
# If exists, overwrite — safe to re-run.
#
# Usage:
#   ./configure.sh \
#     --host 192.168.75.128 \
#     --port 4222
#   ./configure.sh \
#     --host 192.168.75.128
#
# Required Parameters:
#   (none)
#
# Optional Parameters:
#   --host <host>
#       Address printed in NATS_URL (default: `127.0.0.1`).
#       Use the target server IP/hostname when clients connect remotely.
#         Example host values: `192.168.75.128`, `nats.example.com`, `127.0.0.1`
#   --port <port>
#       Listen port written to nats.conf (default: `4222`).
#         Example port values: `4222`, `4223`, `14222`
#
# Override Parameters:
#   RHCTL_NATS_HOST=<host>
#       Same as `--host`.
#   RHCTL_NATS_PORT=<port>
#       Same as `--port`.
#
# Since : 1.0.3
# Date  : Sep 27, 2026
# ************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

RHCTL_NATS_HOST="${RHCTL_NATS_HOST:-127.0.0.1}"
RHCTL_NATS_PORT="${RHCTL_NATS_PORT:-4222}"
CONFIG_FILE="/etc/nats/nats.conf"
NEED_RESTART=false

while [ "$#" -gt 0 ]; do
  case "$1" in
    --host)
      RHCTL_NATS_HOST="$2"
      shift 2
      ;;
    --port)
      RHCTL_NATS_PORT="$2"
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
      echo "[ERROR] Unexpected argument: $1"
      exit 1
      ;;
  esac
done

# ========================================================================= Methods

log() { echo "[INFO] $*"; }
warn() { echo "[WARN] $*"; }

# Update a top-level `key: value` in nats.conf. Returns 0 when changed.
set_nats_kv() {
  local key="$1"
  local value="$2"
  local file="$3"
  local expected="${key}: ${value}"
  local current

  current=$(grep -E "^[[:space:]]*${key}[[:space:]]*:" "$file" | head -1 || true)
  if [ -n "$current" ]; then
    local normalized
    normalized=$(echo "$current" | sed 's/^[[:space:]]*//;s/[[:space:]]*:[[:space:]]*/: /;s/[[:space:]]\+/ /g')
    if [ "$normalized" = "$expected" ]; then
      log "nats.conf '${key}' already set to ${value}"
      return 1
    fi
    sudo sed -i -E "s|^[[:space:]]*${key}[[:space:]]*:.*|${expected}|" "$file"
    log "Updated nats.conf '${key}' = ${value}"
    return 0
  fi

  if grep -qE "^[[:space:]]*#[[:space:]]*${key}[[:space:]]*:" "$file"; then
    sudo sed -i -E "s|^[[:space:]]*#[[:space:]]*${key}[[:space:]]*:.*|${expected}|" "$file"
    log "Uncommented nats.conf '${key}' = ${value}"
    return 0
  fi

  # Insert before jetstream block when present; otherwise append.
  if grep -qE '^[[:space:]]*jetstream[[:space:]]*\{' "$file"; then
    sudo sed -i -E "/^[[:space:]]*jetstream[[:space:]]*\{/i ${expected}" "$file"
  else
    echo "$expected" | sudo tee -a "$file" >/dev/null
  fi
  log "Added nats.conf '${key}' = ${value}"
  return 0
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
  sudo ufw allow "${port}/tcp" comment 'NATS JetStream (rhctl)'
  log "Allowed ${port}/tcp in ufw"
}

# ========================================================================= Config

if ! command -v nats-server &>/dev/null; then
  echo "[ERROR] nats-server not found. Run install.sh first."
  exit 1
fi

if [ ! -f "$CONFIG_FILE" ]; then
  echo "[ERROR] Missing ${CONFIG_FILE}. Run install.sh first."
  exit 1
fi

if ! [[ "$RHCTL_NATS_PORT" =~ ^[0-9]+$ ]] || [ "$RHCTL_NATS_PORT" -lt 1 ] || [ "$RHCTL_NATS_PORT" -gt 65535 ]; then
  echo "[ERROR] Invalid --port: ${RHCTL_NATS_PORT}"
  exit 1
fi

# Listen on all interfaces so remote clients can connect.
if set_nats_kv "host" "0.0.0.0" "$CONFIG_FILE"; then NEED_RESTART=true; fi
if set_nats_kv "port" "${RHCTL_NATS_PORT}" "$CONFIG_FILE"; then NEED_RESTART=true; fi

configure_firewall "$RHCTL_NATS_PORT"

if [ "$NEED_RESTART" = true ]; then
  log "Restarting NATS to apply config"
  sudo systemctl restart nats
else
  log "No nats.conf changes requiring restart"
fi

if ! systemctl is-active --quiet nats; then
  warn "NATS service is not active — attempting start"
  sudo systemctl start nats
fi

if ! systemctl is-active --quiet nats; then
  echo "[ERROR] NATS service failed to start."
  sudo systemctl status nats --no-pager || true
  exit 1
fi

log "NATS service is running"

if command -v ss &>/dev/null; then
  if ss -tln 2>/dev/null | grep -qE "0\\.0\\.0\\.0:${RHCTL_NATS_PORT}|\\[::\\]:${RHCTL_NATS_PORT}|\\*:${RHCTL_NATS_PORT}"; then
    log "NATS is listening on all interfaces (port ${RHCTL_NATS_PORT})"
  else
    warn "NATS may not be listening on all interfaces yet"
    ss -tln 2>/dev/null | grep "${RHCTL_NATS_PORT}" || true
  fi
fi

# ========================================================================= Output

NATS_URL="nats://${RHCTL_NATS_HOST}:${RHCTL_NATS_PORT}"

echo "============================================="
echo "[INFO] NATS JetStream connection"
echo "[INFO]   NATS_HOST=${RHCTL_NATS_HOST}"
echo "[INFO]   NATS_PORT=${RHCTL_NATS_PORT}"
echo "[INFO]   NATS_URL=${NATS_URL}"
echo "[INFO] Test:"
echo "[INFO]   nats server check --server ${NATS_URL}"
echo "============================================="
