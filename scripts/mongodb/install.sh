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
# Install MongoDB.
#
# Author: Craig Brown
# Since : 1.0.1
# Date  : Feb 8, 2026
# ************************************************************************************

echo "[INFO] Install prerequisites"

sudo apt update
sudo apt install -y gnupg curl

echo "[INFO] Import the MongoDB public GPG key"

curl -fsSL https://www.mongodb.org/static/pgp/server-7.0.asc | \
   sudo gpg -o /usr/share/keyrings/mongodb-server-7.0.gpg \
   --dearmor

echo "[INFO] Create the list file (adjust version & codename if needed)"
# For Ubuntu 24.04 (noble)
# For Ubuntu 22.04 (jammy) → replace noble with jammy
echo "deb [ arch=amd64,arm64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg ] https://repo.mongodb.org/apt/ubuntu noble/mongodb-org/8.0 multiverse" | \
   sudo tee /etc/apt/sources.list.d/mongodb-org-8.0.list

echo "[INFO] Update package list again"
sudo apt update

echo "[INFO] Install the latest stable version"
sudo apt install -y mongodb-org

# (or pin a specific version e.g. 8.0.3)
# sudo apt install -y mongodb-org=8.0.3 mongodb-org-database=8.0.3 mongodb-org-server=8.0.3 mongodb-mongosh mongodb-org-mongos=8.0.3 mongodb-org-tools=8.0.3