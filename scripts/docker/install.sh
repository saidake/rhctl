#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
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
# Run this command in the current project directory to install Docker and Docker Compose.
#
# Example Success Output:
# 
#    ...
#    [INFO] Docker installed successfully: Docker version 28.3.2, build 578ccf6
#    ...
#    [INFO] Docker Compose installed successfully: Docker Compose version v2.38.2
#    [INFO] Installation complete.
#
# Since : 1.0.0
# Date  : October 16, 2025
# ************************************************************************************

set -e

echo "[INFO] Checking and syncing system time..."
if ! command -v ntpdate >/dev/null 2>&1; then
  sudo apt-get update -y || true
  sudo apt-get install -y ntpdate
  echo "[INFO] ntpdate installed successfully."
fi

if sudo ntpdate -u time.google.com; then
  echo "[INFO] Time sync successful."
else
  echo "[WARN] Time sync failed. Continuing with existing system time."
fi

echo "[INFO] Updating package index..."
if ! sudo apt-get update; then
  echo "[ERROR] apt-get update failed. Check your network or time settings."
  exit 1
fi

echo "[INFO] Installing prerequisites..."
sudo apt-get install -y ca-certificates curl gnupg lsb-release

# Check if Docker GPG keyring exists
KEYRING_PATH="/usr/share/keyrings/docker-archive-keyring.gpg"
if [[ -f "$KEYRING_PATH" ]]; then
  echo "[INFO] Docker GPG keyring already exists, skipping import."
else
  echo "[INFO] Adding Docker's official GPG key..."
  curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor --yes --batch -o "$KEYRING_PATH"
fi

# Check if docker.list repo file exists
REPO_FILE="/etc/apt/sources.list.d/docker.list"
if [[ -f "$REPO_FILE" ]]; then
  echo "[INFO] Docker repository list already exists, skipping."
else
  echo "[INFO] Setting up the Docker repository..."
  echo \
    "deb [arch=$(dpkg --print-architecture) signed-by=$KEYRING_PATH] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | \
    sudo tee "$REPO_FILE" > /dev/null
fi

echo "[INFO] Updating package index again..."
sudo apt-get update

echo "[INFO] Installing Docker Engine..."
sudo apt-get install -y docker-ce docker-ce-cli containerd.io

echo "[INFO] Verifying Docker installation..."
if sudo docker --version >/dev/null 2>&1; then
  echo "[INFO] Docker installed successfully: $(sudo docker --version)"
else
  echo "[ERROR] Docker installation failed."
  exit 1
fi

echo "[INFO] Downloading latest Docker Compose binary..."
DOCKER_COMPOSE_LATEST=$(curl -fsSL https://api.github.com/repos/docker/compose/releases/latest | grep -Po '"tag_name": "\K.*?(?=")')
if [[ -z "$DOCKER_COMPOSE_LATEST" ]]; then
  echo "[WARN] Could not determine latest Docker Compose version, using default 2.20.2"
  DOCKER_COMPOSE_LATEST="v2.20.2"
fi

COMPOSE_BIN="/usr/local/bin/docker-compose"
if [[ -x "$COMPOSE_BIN" ]]; then
  echo "[INFO] Docker Compose binary already exists, skipping download."
else
  sudo curl -L "https://github.com/docker/compose/releases/download/${DOCKER_COMPOSE_LATEST}/docker-compose-$(uname -s)-$(uname -m)" -o "$COMPOSE_BIN"
  sudo chmod +x "$COMPOSE_BIN"
fi

echo "[INFO] Verifying Docker Compose installation..."
if docker-compose --version >/dev/null 2>&1; then
  echo "[INFO] Docker Compose installed successfully: $(docker-compose --version)"
else
  echo "[ERROR] Docker Compose installation failed."
  exit 1
fi

echo "[INFO] Installation complete."
