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
# Uninstall script for RocksDB
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
INSTALL_PREFIX="/usr/local/rocksdb"
# ================================================================== Logic

set -e

echo "[INFO] Starting uninstallation of RocksDB from $INSTALL_PREFIX ..."

# Step 1: Remove installed files
if [ -d "$INSTALL_PREFIX" ]; then
  echo "[INFO] Removing installation directory: $INSTALL_PREFIX"
  sudo rm -rf "$INSTALL_PREFIX"
else
  echo "[WARN] Installation directory $INSTALL_PREFIX does not exist. Skipping."
fi

# Step 2: Remove LD_LIBRARY_PATH entry from ~/.bashrc
BASHRC="$HOME/.bashrc"
if grep -q "$INSTALL_PREFIX/lib" "$BASHRC"; then
  echo "[INFO] Removing LD_LIBRARY_PATH entry from $BASHRC"
  sed -i "\|$INSTALL_PREFIX/lib|d" "$BASHRC"
else
  echo "[INFO] No LD_LIBRARY_PATH entry found in $BASHRC. Skipping."
fi

# Step 3: Refresh dynamic linker cache
echo "[INFO] Running ldconfig to refresh shared library cache..."
sudo ldconfig

echo "[SUCCESS] RocksDB has been successfully uninstalled from $INSTALL_PREFIX"