#!/bin/bash
# ************************************************************************************
# Copyright 2022-2025 the original author or authors.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
# ************************************************************************************
# Stop LocalStack Service.
#
# Prerequisites:
#    1. Configure variables in `scripts/AAA/config/server.sh`:
#         - REMOTE_HOST
#         - REMOTE_USER
#         - REMOTE_SSH_PORT (default: 22)
#         - REMOTE_PWD
#    2. **Docker** and **Docker Compose** have been installed on the remote server. 
#        (see [Docker and Docker Compose / Installing on a Remote Linux Host](#installing-on-a-remote-linux-host)).
#
# Author: Craig Brown
# Since : 1.3.1
# Date  : July 20, 2025
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
