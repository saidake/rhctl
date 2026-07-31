#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 Craig Brown and rhctl Contributors
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.
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
