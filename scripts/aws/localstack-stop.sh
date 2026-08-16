#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
# ************************************************************************************
# Stop LocalStack Service.
#
# Prerequisites:
#    1. **Docker** and **Docker Compose** have been installed on the remote server. 
#        (see [Docker and Docker Compose / Installing on a Remote Linux Host](#installing-on-a-remote-linux-host)).
#
# Since : 1.0.0
# Date  : October 16, 2025
# ************************************************************************************

COMPOSE_DIR="/opt/sandbox/aws"
COMPOSE_FILE="$COMPOSE_DIR/docker-compose.yml"

echo "[INFO] Stopping LocalStack services using docker-compose..."

if [[ ! -f "$COMPOSE_FILE" ]]; then
  echo "[ERROR] docker-compose.yml not found at $COMPOSE_FILE"
  exit 1
fi

cd "$COMPOSE_DIR" || {
  echo "[ERROR] Failed to change directory to $COMPOSE_DIR"
  exit 1
}

docker-compose stop localstack
if [[ $? -eq 0 ]]; then
  echo "[INFO] LocalStack stopped successfully."
else
  echo "[WARN] Failed to stop LocalStack, you may want to check manually."
fi
