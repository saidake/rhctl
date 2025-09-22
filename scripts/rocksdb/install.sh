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
# Install RocksDB.
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
DOWNLOAD_DIR="/opt/sandbox/rocksdb"
FORCE_DELETE_EXISTING_DOWNLOAD_DIR=false
INSTALL_PREFIX="/usr/local/rocksdb"  
# ================================================================== Logic

set -e

echo "[INFO] Checking and installing required dependencies..."

# List of required packages
required_packages=(
  build-essential
  libsnappy-dev
  zlib1g-dev
  libbz2-dev
  libgflags-dev
  cmake
  git
)

# Check and install missing packages
missing_packages=()
for pkg in "${required_packages[@]}"; do
  if ! dpkg -s "$pkg" >/dev/null 2>&1; then
    missing_packages+=("$pkg")
  fi
done

if [ ${#missing_packages[@]} -ne 0 ]; then
  echo "[INFO] Missing packages detected: ${missing_packages[*]}"
  sudo apt-get update
  sudo apt-get install -y "${missing_packages[@]}"
else
  echo "[INFO] All required dependencies are already installed."
fi

# Clone RocksDB if needed
if [ -d "$DOWNLOAD_DIR" ] && [ "$(ls -A "$DOWNLOAD_DIR")" ]; then
  if [ "$FORCE_DELETE_EXISTING_DOWNLOAD_DIR" = true ]; then
    echo "[INFO] Deleting existing directory $DOWNLOAD_DIR ..."
    rm -rf "$DOWNLOAD_DIR"
    git clone https://github.com/facebook/rocksdb.git "$DOWNLOAD_DIR"
  else
    echo "[INFO] Using existing RocksDB directory at $DOWNLOAD_DIR"
  fi
else
  echo "[INFO] Cloning RocksDB into $DOWNLOAD_DIR ..."
  git clone https://github.com/facebook/rocksdb.git "$DOWNLOAD_DIR"
fi

cd "$DOWNLOAD_DIR"

echo "[INFO] Building RocksDB..."
mkdir -p build && cd build
cmake .. -DCMAKE_INSTALL_PREFIX="$INSTALL_PREFIX"
make -j"$(nproc)"

echo "[INFO] Installing RocksDB to $INSTALL_PREFIX ..."
sudo make install

echo "[INFO] Configuring dynamic linker..."
echo "export LD_LIBRARY_PATH=$INSTALL_PREFIX/lib:\$LD_LIBRARY_PATH" >> ~/.bashrc
export LD_LIBRARY_PATH="$INSTALL_PREFIX/lib:$LD_LIBRARY_PATH"

sudo ldconfig
echo "[SUCCESS] RocksDB installed to $INSTALL_PREFIX"