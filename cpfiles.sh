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
# Automates uploading files from `ASSETS_ROOT` to mapped remote paths, according to 
# rules in `PROPERTIES_FILE`.
#
# Prerequisites:
#   1. `sshpass` is installed locally.
#   2. Make sure the current bash file has execution privileges: `chmod +x scripts/cpfiles.sh`
#   3. Configure server variables in `scripts/AAA/config/server.sh`:
#        - REMOTE_HOST
#        - REMOTE_USER
#        - REMOTE_SSH_PORT (default: 22)
#        - REMOTE_PWD
#
# Examples (run directly for easy start, using default settings):
#   * `./scripts/cpfiles.sh`
#
#       Copies `example1.txt` to the remote directory `~/examples`,
#       and copies `example2.txt` and `example3.txt` from `scripts/AAA/assets/exampledir` 
#       to the remote directory `~/examples/targetdir` on the test server.
#   * `./scripts/cpfiles.sh ./scripts/AAA/assets/example-env.sh`
#
#       Use the specified env configuration to copy `example2.txt` and `example3.txt` 
#       from `scripts/AAA/assets/exampledir` to the remote directory
#       `~/examples/targetdir2` on the test server.
#
# Usage:
#   * `./scripts/cpfiles.sh [<env.sh>]`
#
#      You can define script options in a specified `env.sh` to override the default options in this script.
#
# Script Options (variables in this script):
#   * USE_RSYNC    : (true/false) Use `rsync` for uploading instead of `scp`.
#   * USE_SUDO     : (true/false) If true, these commands will be executed with sudo privileges on the remote machine.  
#        Reminder: With sudo, `~` points to `/root`, not the current user's home.
#   * SILENT       : (true/false) If true, disables all overwrite confirmation prompts (auto-approve).
#   * PROPERTIES_FILE   : Copies the folder contents or files corresponding to the keys in the properties file
#       to the remote directories specified by the values.
#   * ASSETS_ROOT       : The base directory where the relative paths (keys) from PROPERTIES_FILE are located.
# 
#
# Global Environment Variables:
#   * ROOT : The absolute path of the scripts directory.
#
# Author: Craig Brown
# Since : 1.1.0
# Date  : July 8, 2025
# ************************************************************************************
source "$(dirname "${BASH_SOURCE[0]}")/AAA/config/global.sh"

# ================================================================== Required Configurations
# Import global environment variables
source "$ROOT/AAA/config/server.sh"
# ================================================================== Default Configurations
USE_SUDO=false
# Use 'rsync' instead of 'scp'
USE_RSYNC=false
# Ask warning messages
SILENT=false
# Load file-to-directory mappings from properties file
PROPERTIES_FILE="$ROOT/AAA/config/path-mapping.properties"
# Assets
ASSETS_ROOT="$ROOT/AAA/assets"

# ================================================================== Functions
if [[ -n "$1" ]]; then
  if [[ -f "$1" ]]; then
    echo "[INFO] Loading override environment variables from $1 ..."
    # shellcheck disable=SC1090
    source "$1"
  else
    echo "[ERROR] Environment file '$1' not found."
    exit 1
  fi
fi


source "$ROOT/AAA/common/functions.sh"
source "$ROOT/AAA/common/property_transfer.sh"
# ================================================================== Logic
check_ssh_connection
trust_host
check_sshpass_installed
upload_files_by_properties "$PROPERTIES_FILE" "$ASSETS_ROOT" $USE_RSYNC $SILENT