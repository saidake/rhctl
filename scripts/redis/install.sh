#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Install Redis.
# Port: 6379
#
# Since : 1.0.1
# Date  : Feb 8, 2026
# ************************************************************************************

# Check if Redis is already installed
if command -v redis-server &> /dev/null; then
    REDIS_VERSION=$(redis-server --version 2>&1 | grep -oP 'v=\K[\d.]+' | head -1)
    echo "[INFO] Redis is already installed (version: $REDIS_VERSION)"
    echo "[INFO] Skipping reinstallation"
    
    # Check if Redis service is running
    if systemctl is-active --quiet redis-server; then
        echo "[INFO] Redis service is currently running"
    else
        echo "[WARN] Redis service is not running. Starting it now..."
        sudo systemctl start redis-server
    fi
    
    echo "[INFO] To test Redis, run: redis-cli ping"
    exit 0
fi

echo "[INFO] Redis not found. Proceeding with installation..."

echo "[INFO] Install prerequisites"

sudo apt update
sudo apt install -y lsb-release curl gpg

echo "[INFO] Add the Redis repository key"

curl -fsSL https://packages.redis.io/gpg | sudo gpg --dearmor -o /usr/share/keyrings/redis-archive-keyring.gpg

echo "[INFO] Add the Redis repository to APT sources"

echo "deb [signed-by=/usr/share/keyrings/redis-archive-keyring.gpg] https://packages.redis.io/deb $(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/redis.list

echo "[INFO] Update package list"

sudo apt update

echo "[INFO] Install Redis"

sudo apt install -y redis

echo "[INFO] Enable Redis to start on boot"

sudo systemctl enable redis-server

echo "[INFO] Start Redis service"

sudo systemctl start redis-server

echo "[INFO] Check Redis status"

sudo systemctl status redis-server --no-pager

echo "[INFO] Redis installation completed successfully"
echo "[INFO] Installed Redis version: $(redis-server --version)"
echo "[INFO] To test Redis, run: redis-cli ping"