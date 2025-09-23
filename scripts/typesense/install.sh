#!/bin/bash
# ************************************************************************************
# Copyright 2022-2025 the original author or authors.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      https://www.apache.org/licenses/LICENSE-2.0
# ************************************************************************************
# Install Typesense.
# 
# Prerequisites:
#    1. Configure variables in `scripts/AAA/config/server.sh`:
#         - REMOTE_HOST
#         - REMOTE_USER
#         - REMOTE_SSH_PORT (default: 22)
#         - REMOTE_PWD
#
# Author: Craig Brown
# Since: 1.3.4
# Date: August 15, 2025
# ************************************************************************************
# ================================================================== Required Configurations
INSTALL_DIR="/opt/sandbox/typesense"
FORCE_DELETE_EXISTING_INSTALL_DIR=false
TYPESENSE_VERSION="29.0"
TYPESENSE_FILENAME="typesense-server-${TYPESENSE_VERSION}-linux-amd64"
TYPESENSE_URL="https://dl.typesense.org/releases/${TYPESENSE_VERSION}/${TYPESENSE_FILENAME}.tar.gz"
TARBALL_PATH="/tmp/typesense-${TYPESENSE_VERSION}.tar.gz"

# ================================================================== Logic
set -e

echo "[INFO] Installing Typesense ${TYPESENSE_VERSION}..."

# Step 1: Prepare INSTALL_DIR
if [ -d "$INSTALL_DIR" ]; then
  if [ "$(ls -A "$INSTALL_DIR")" ]; then
    if [ "$FORCE_DELETE_EXISTING_INSTALL_DIR" = true ]; then
      echo "[INFO] Force deleting existing directory $INSTALL_DIR ..."
      sudo rm -rf "$INSTALL_DIR"
      sudo mkdir -p "$INSTALL_DIR"
      sudo chown "$USER:$USER" "$INSTALL_DIR"
    else
      echo "[INFO] Directory $INSTALL_DIR already exists and is not empty. Skipping installation."
      echo "[USAGE] Run with:"
      echo "  $INSTALL_DIR/typesense-server --help"
      exit 0
    fi
  else
    echo "[INFO] Directory $INSTALL_DIR exists and is empty. Proceeding."
  fi
else
  echo "[INFO] Creating install directory $INSTALL_DIR ..."
  sudo mkdir -p "$INSTALL_DIR"
fi

# Step 2: Download tarball (if not already downloaded)
if [ ! -f "$TARBALL_PATH" ]; then
  echo "[INFO] Downloading $TYPESENSE_FILENAME ..."
  curl -L "$TYPESENSE_URL" -o "$TARBALL_PATH"
else
  echo "[INFO] Using cached tarball at $TARBALL_PATH"
fi

# Step 3: Extract only if binary doesn't exist
if [ ! -f "$INSTALL_DIR/typesense-server" ]; then
  echo "[INFO] Extracting archive to $INSTALL_DIR ..."
  tar -xzf "$TARBALL_PATH" -C "$INSTALL_DIR"
  chmod +x "$INSTALL_DIR/typesense-server"
else
  echo "[INFO] typesense-server binary already exists. Skipping extraction."
fi

# Optional: Clean tarball (you can keep it for reuse if needed)
# rm "$TARBALL_PATH"

echo "[SUCCESS] Typesense ${TYPESENSE_VERSION} installed in $INSTALL_DIR"
echo "[USAGE] Run with:"
echo "  $INSTALL_DIR/typesense-server --help"