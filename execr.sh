#!/bin/bash
# ************************************************************************************
# Copyright 2012-2025 the original author or authors.
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
# This script executes a local bash file on the remote server.
#
# Prerequisites:
#   1. `sshpass` is installed locally.
#   2. Make sure the current bash file has execution privileges: `chmod +x scripts/execr.sh`
#   3. Configure variables in `scripts/AAA/config/server.sh`:
#        - REMOTE_HOST
#        - REMOTE_USER
#        - REMOTE_SSH_PORT (default: 22)
#        - REMOTE_PWD
#
# Examples (run directly for easy start, using default settings):
#   * `./scripts/execr.sh ./scripts/AAA/assets/example-bash.sh`
#
#      Execute the bash file `./scripts/AAA/assets/example-bash.sh` on your remote server.
#
# Usage:
#   * `./scripts/exec.sh <local-bash-file-path>`
# 
#     The first parameter `<local-bash-file-path>` is a local Bash file, which will be
#     executed directly on the remote server.
#
# Script Options (variables in this script):
#   * USE_RSYNC    : (true/false) Use 'rsync' for uploading instead of 'scp'.
#   * USE_SUDO     : (true/false) If true, the bash script will be executed with sudo privileges on the remote machine.
#
#       In sudo mode, the script is first copied to the remote server before execution.  
#       In non-sudo mode, it is executed directly on the remote server via SSH.  
#       Reminder: With sudo, `~` points to `/root`, not the current user's home.
#   * SILENT       : (true/false) If true, disables all confirmation prompts (auto-approve).
#   * LOCAL_BASH_FILE       : Local bash file to execute
#   * REMOTE_TMP_BASH_FILE  : The local bash script will be uploaded to this path on the remote server.
#   * REMOTE_WORK_PATH      : The working directory on the remote server where the script will be executed.
#                            
# Global Env:
#   * ROOT : The absolute path of scripts directory.
#
# Author: Craig Brown
# Since: 1.1.0
# Date: Oct 4, 2024
# ************************************************************************************
source "$(dirname "${BASH_SOURCE[0]}")/AAA/config/global.sh"

# ================================================================== Required Configurations
# Import global environment variables
source "$ROOT/AAA/config/server.sh"

# ================================================================== Default Configurations
LOCAL_BASH_FILE="$1"     # Local bash file to execute
REMOTE_WORK_PATH="~"

USE_SUDO=true
SILENT=false
USE_RSYNC=false
# ================================================================== Validate Input
source "$ROOT/AAA/common/functions.sh"
source "$ROOT/AAA/common/upload.sh"
check_ssh_connection
trust_host
check_sshpass_installed

if [[ -z "$LOCAL_BASH_FILE" ]]; then
  echo "[ERROR] No bash script file provided. Please pass the script file path as an argument."
  exit 1
fi

if [[ ! -f "$LOCAL_BASH_FILE" ]]; then
  echo "[ERROR] File does not exist: $LOCAL_BASH_FILE"
  exit 1
fi

# ================================================================== Execute Remotely

# Use sshpass to pass the password
#remote_execute "cd $REMOTE_WORK_PATH && bash -l -c 'bash -s'" < "$LOCAL_BASH_FILE"
#remote_execute "cd $REMOTE_WORK_PATH && echo \"$REMOTE_PWD\" | sudo -S bash -l -c 'bash -s'" < "$LOCAL_BASH_FILE"
#remote_execute "cd $REMOTE_WORK_PATH && sudo -S bash -l -s" < "$LOCAL_BASH_FILE"
#remote_execute "cd $REMOTE_WORK_PATH && echo \"$REMOTE_PWD\" | sudo -S bash -l -s" < "$LOCAL_BASH_FILE"
#remote_execute "cd $REMOTE_WORK_PATH && echo \"$REMOTE_PWD\" | sudo -S bash -l -s" < "$LOCAL_BASH_FILE"
#sshpass -p "$REMOTE_PWD" ssh -p "$REMOTE_SSH_PORT" "$REMOTE_USER@$REMOTE_HOST" << EOF
#echo "$REMOTE_PWD" | sudo -S pwd
#pwd
#EOF
#remote_execute "cd $REMOTE_WORK_PATH && echo \"$REMOTE_PWD\" | sudo -S bash -l -s" < "$LOCAL_BASH_FILE"
# upload_file_to_file "$LOCAL_BASH_FILE" "$REMOTE_TMP_BASH_FILE" $USE_RSYNC $SILENT
#remote_execute "echo \"$REMOTE_PWD\" | sudo -S bash $REMOTE_TMP_BASH_FILE && rm -f $REMOTE_TMP_BASH_FILE"
# remote_execute "echo \"$REMOTE_PWD\" | sudo -S bash -c 'echo; echo \"[INFO] Remote execution output: \"; bash \"$REMOTE_TMP_BASH_FILE\"; rm -f \"$REMOTE_TMP_BASH_FILE\"'"
remote_execute_local_script "$LOCAL_BASH_FILE" "$REMOTE_WORK_PATH" $USE_RSYNC $SILENT