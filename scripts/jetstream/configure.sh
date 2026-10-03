#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Configure an installed NATS JetStream server for remote client access.
#
# Ensures listen host/port in nats.conf, opens the firewall port (ufw when
# present), restarts the service when config changes, and ensures the NATS CLI
# is installed.
#
# If exists, overwrite — safe to re-run.
#
# Usage:
#   ./configure.sh 
#   ./configure.sh --port 4222
#
# Required Parameters:
#   (none)
#
# Optional Parameters:
#   --port <port>
#       Listen port written to nats.conf (default: `4222`).
#         Example port values: `4222`, `4223`, `14222`
#
# Override Parameters:
#   RHCTL_NATS_PORT=<port>
#       Same as `--port`.
#
#   RHCTL_NATS_CLI_VERSION=<version>
#       NATS CLI version to install when the CLI is missing.
#       Defaults to the latest GitHub release.
#
# Since : 1.0.4
# Date  : Oct 3, 2026
# ************************************************************************************

set -euo pipefail

# ========================================================================= Parameter

RHCTL_NATS_PORT="${RHCTL_NATS_PORT:-4222}"
CONFIG_FILE="/etc/nats/nats.conf"
NEED_RESTART=false

# Placeholder in printed NATS_URL (replace with the client-facing host).
NATS_HOST_PLACEHOLDER='<host>'

while [ "$#" -gt 0 ]; do
  case "$1" in
    --port)
      [ "$#" -ge 2 ] || {
        echo "[ERROR] --port requires a value."
        exit 1
      }

      RHCTL_NATS_PORT="$2"
      shift 2
      ;;

    -h|--help)
      sed -n '2,45p' "$0"
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

install_nats_cli() {
  if command -v nats &>/dev/null; then
    log "NATS CLI already installed: $(nats --version)"
    return 0
  fi

  log "NATS CLI not found — installing"

  if ! command -v curl &>/dev/null; then
    echo "[ERROR] curl is required to install the NATS CLI."
    exit 1
  fi

  if ! command -v unzip &>/dev/null; then
    log "unzip not found — attempting to install automatically"
    
    if command -v apt-get &>/dev/null; then
      sudo apt-get update -qq && sudo apt-get install -y unzip
    elif command -v dnf &>/dev/null; then
      sudo dnf install -y unzip
    elif command -v yum &>/dev/null; then
      sudo yum install -y unzip
    elif command -v apk &>/dev/null; then
      sudo apk add unzip
    elif command -v zypper &>/dev/null; then
      sudo zypper install -y unzip
    elif command -v pacman &>/dev/null; then
      sudo pacman -Sy --noconfirm unzip
    else
      echo "[ERROR] Unsupported package manager. Please install 'unzip' manually."
      exit 1
    fi

    if ! command -v unzip &>/dev/null; then
      echo "[ERROR] Failed to install 'unzip' automatically."
      exit 1
    fi
    log "unzip installed successfully"
  fi

  local OS
  local ARCH
  local VERSION
  local TMP_DIR
  local RELEASE_JSON
  local DOWNLOAD_URL
  local ARCHIVE
  local BINARY

  OS="$(uname -s)"

  if [ "${OS}" != "Linux" ]; then
    echo "[ERROR] Unsupported operating system for NATS CLI: ${OS}"
    exit 1
  fi

  case "$(uname -m)" in
    x86_64|amd64)
      ARCH="amd64"
      ;;

    aarch64|arm64)
      ARCH="arm64"
      ;;

    armv7l|armv7)
      ARCH="arm7"
      ;;

    *)
      echo "[ERROR] Unsupported architecture for NATS CLI: $(uname -m)"
      exit 1
      ;;
  esac

  TMP_DIR="$(mktemp -d)"

  trap 'rm -rf "${TMP_DIR}"' RETURN

  if [ -n "${RHCTL_NATS_CLI_VERSION:-}" ]; then
    VERSION="${RHCTL_NATS_CLI_VERSION}"

    DOWNLOAD_URL="https://github.com/nats-io/natscli/releases/download/v${VERSION}/nats-${VERSION}-linux-${ARCH}.zip"
  else
    log "Resolving latest NATS CLI release"

    RELEASE_JSON="$(
      curl -fsSL \
        -H 'Accept: application/vnd.github+json' \
        https://api.github.com/repos/nats-io/natscli/releases/latest
    )"

    DOWNLOAD_URL="$(
      printf '%s\n' "${RELEASE_JSON}" |
        grep -oE '"browser_download_url":[[:space:]]*"[^"]+"' |
        sed -E 's/^"browser_download_url":[[:space:]]*"//;s/"$//' |
        grep -E "/nats-[0-9.]+-linux-${ARCH}\.zip$" |
        head -1
    )"

    if [ -z "${DOWNLOAD_URL}" ]; then
      echo "[ERROR] Failed to find a NATS CLI release for linux-${ARCH}."
      exit 1
    fi
  fi

  log "Downloading NATS CLI"
  log "  ${DOWNLOAD_URL}"

  ARCHIVE="${TMP_DIR}/nats.zip"

  curl -fL \
    --retry 3 \
    --retry-delay 2 \
    "${DOWNLOAD_URL}" \
    -o "${ARCHIVE}"

  unzip -q "${ARCHIVE}" -d "${TMP_DIR}"

  BINARY="$(find "${TMP_DIR}" -type f -name nats -perm -111 | head -1 || true)"

  if [ -z "${BINARY}" ]; then
    echo "[ERROR] NATS CLI binary was not found in the downloaded archive."
    exit 1
  fi

  sudo install -m 0755 "${BINARY}" /usr/local/bin/nats

  if ! command -v nats &>/dev/null; then
    echo "[ERROR] Failed to install NATS CLI."
    exit 1
  fi

  log "NATS CLI installed: $(nats --version)"
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

if ! [[ "$RHCTL_NATS_PORT" =~ ^[0-9]+$ ]] ||
   [ "$RHCTL_NATS_PORT" -lt 1 ] ||
   [ "$RHCTL_NATS_PORT" -gt 65535 ]; then
  echo "[ERROR] Invalid --port: ${RHCTL_NATS_PORT}"
  exit 1
fi

# ========================================================================= NATS CLI

install_nats_cli

# ========================================================================= NATS Server

# Listen on all interfaces so remote clients can connect.
if set_nats_kv "host" "0.0.0.0" "$CONFIG_FILE"; then
  NEED_RESTART=true
fi

if set_nats_kv "port" "${RHCTL_NATS_PORT}" "$CONFIG_FILE"; then
  NEED_RESTART=true
fi

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
  if ss -tln 2>/dev/null |
      grep -qE "0\\.0\\.0\\.0:${RHCTL_NATS_PORT}|\\[::\\]:${RHCTL_NATS_PORT}|\\*:${RHCTL_NATS_PORT}"; then
    log "NATS is listening on all interfaces (port ${RHCTL_NATS_PORT})"
  else
    warn "NATS may not be listening on all interfaces yet"
    ss -tln 2>/dev/null | grep "${RHCTL_NATS_PORT}" || true
  fi
fi

# ========================================================================= Output

NATS_URL="nats://${NATS_HOST_PLACEHOLDER}:${RHCTL_NATS_PORT}"

echo "============================================="
echo "[INFO] NATS JetStream connection"
echo "[INFO]   NATS_HOST=${NATS_HOST_PLACEHOLDER}"
echo "[INFO]   NATS_PORT=${RHCTL_NATS_PORT}"
echo "[INFO]   NATS_URL=${NATS_URL}"
echo "[INFO] NATS CLI"
echo "[INFO]   $(nats --version)"
echo "[INFO] Test:"
echo "[INFO]   nats server check --server ${NATS_URL}"
echo "============================================="