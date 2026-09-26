#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Install NATS Server with JetStream and configure a systemd service.
# Default Port: 4222
#
# If exists, skip — safe to re-run.
#
# Usage:
#   ./install.sh
#   ./install.sh --nats-version v2.12.5
#
# Required Parameters:
#   (none)
#
# Optional Parameters:
#   --nats-version <version>
#       NATS Server release tag to install when missing (default: `latest`).
#         Example version values: `latest`, `v2.12.5`, `v2.10.12`
#
# Override Parameters:
#   RHCTL_NATS_VERSION=<version>
#       Same as `--nats-version`.
#         Example version values: `latest`, `v2.12.5`, `v2.10.12`
#
# Since : 1.0.2
# Date  : Sep 26, 2026
# ************************************************************************************
# Commands:
#   systemctl status nats
#   systemctl start nats
#   systemctl stop nats

set -e

NATS_VERSION="${RHCTL_NATS_VERSION:-latest}"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/nats"
DATA_DIR="/var/lib/nats"
CONFIG_FILE="$CONFIG_DIR/nats.conf"
SERVICE_FILE="/etc/systemd/system/nats.service"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --nats-version)
      if [[ -z "${2:-}" ]]; then
        echo "[ERROR] --nats-version requires a value."
        exit 1
      fi
      NATS_VERSION="$2"
      shift 2
      ;;
    -h|--help)
      sed -n '2,40p' "$0"
      exit 0
      ;;
    *)
      echo "[ERROR] Unknown argument: $1"
      exit 1
      ;;
  esac
done

resolve_nats_version() {
  local requested="$1"
  if [[ "$requested" != "latest" ]]; then
    echo "$requested"
    return 0
  fi

  echo "[INFO] Resolving latest NATS Server release..." >&2
  local tag
  tag=$(curl -fsSL https://api.github.com/repos/nats-io/nats-server/releases/latest \
    | grep -Po '"tag_name": "\K.*?(?=")' | head -1)

  if [[ -z "$tag" ]]; then
    echo "[ERROR] Could not determine latest NATS Server version." >&2
    return 1
  fi

  echo "[INFO] Latest NATS Server release: ${tag}" >&2
  echo "$tag"
}

echo "[INFO] Updating package index..."
sudo apt-get update -y

echo "[INFO] Installing prerequisites..."
sudo apt-get install -y curl tar

# ------------------------------------------------
# Install NATS Server
# ------------------------------------------------

if command -v nats-server >/dev/null 2>&1; then
  echo "[INFO] NATS already installed: $(nats-server --version)"
else
  NATS_VERSION="$(resolve_nats_version "$NATS_VERSION")"
  echo "[INFO] Installing NATS Server ${NATS_VERSION}..."

  TMP_DIR=$(mktemp -d)
  trap 'rm -rf "${TMP_DIR}"' EXIT

  curl -L "https://github.com/nats-io/nats-server/releases/download/${NATS_VERSION}/nats-server-${NATS_VERSION}-linux-amd64.tar.gz" \
    -o "${TMP_DIR}/nats-server.tar.gz"

  tar -xzf "${TMP_DIR}/nats-server.tar.gz" -C "${TMP_DIR}"

  NATS_BIN=$(find "${TMP_DIR}" -name "nats-server" -type f | head -1)
  if [[ -z "$NATS_BIN" ]]; then
    echo "[ERROR] nats-server binary not found in archive."
    exit 1
  fi

  sudo mv "$NATS_BIN" "${INSTALL_DIR}/nats-server"
  sudo chmod +x "${INSTALL_DIR}/nats-server"

  if nats-server --version >/dev/null 2>&1; then
    echo "[INFO] NATS installed successfully: $(nats-server --version)"
  else
    echo "[ERROR] NATS installation failed."
    exit 1
  fi
fi

# ------------------------------------------------
# Create system user
# ------------------------------------------------

if id "nats" &>/dev/null; then
  echo "[INFO] User 'nats' already exists."
else
  echo "[INFO] Creating system user 'nats'..."
  sudo useradd --system --no-create-home --shell /usr/sbin/nologin nats
fi

# ------------------------------------------------
# Create directories
# ------------------------------------------------

echo "[INFO] Creating required directories..."

sudo mkdir -p "$CONFIG_DIR"
sudo mkdir -p "$DATA_DIR"

sudo chown -R nats:nats "$DATA_DIR"

# ------------------------------------------------
# Create configuration file
# ------------------------------------------------

if [[ -f "$CONFIG_FILE" ]]; then
  echo "[INFO] NATS config already exists: $CONFIG_FILE"
else
  echo "[INFO] Creating NATS configuration..."

  sudo tee "$CONFIG_FILE" > /dev/null <<EOF
port: 4222

jetstream {
  store_dir: ${DATA_DIR}
  max_mem_store: 1GB
  max_file_store: 10GB
}
EOF

  echo "[INFO] Config file created."
fi

# ------------------------------------------------
# Create systemd service
# ------------------------------------------------

if [[ -f "$SERVICE_FILE" ]]; then
  echo "[INFO] systemd service already exists."
else
  echo "[INFO] Creating systemd service..."

  sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=NATS Server
After=network.target

[Service]
ExecStart=${INSTALL_DIR}/nats-server -c ${CONFIG_FILE}
Restart=always
User=nats
Group=nats
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

  echo "[INFO] systemd service created."

  sudo systemctl daemon-reload
fi

# ------------------------------------------------
# Enable service
# ------------------------------------------------

if systemctl is-enabled --quiet nats; then
  echo "[INFO] NATS service already enabled."
else
  echo "[INFO] Enabling NATS service..."
  sudo systemctl enable nats
fi

# ------------------------------------------------
# Start service
# ------------------------------------------------

if systemctl is-active --quiet nats; then
  echo "[INFO] NATS service already running."
else
  echo "[INFO] Starting NATS service..."
  sudo systemctl start nats
fi

# ------------------------------------------------
# Verify service
# ------------------------------------------------

echo "[INFO] Verifying NATS service..."

if systemctl is-active --quiet nats; then
  echo "[INFO] NATS service is running."
else
  echo "[ERROR] NATS service failed to start."
  exit 1
fi

echo "[INFO] Installation complete."
