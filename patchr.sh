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
# This script safely patches a remote file on a server using one of two modes:
#   1. Upload a local file and replace a target remote file.
#   2. Recover the original remote file from a backup.
#
# Steps (in patch mode):
#   1. Upload a local file (`LOCAL_FILE`) to a temporary location (`REMOTE_UPLOAD_FILE`) on the remote server.
#   2. Backup the target remote file (`REMOTE_FILE`) to a backup path (`REMOTE_BACKUP_FILE`).
#   3. Overwrite the remote target file (`REMOTE_FILE`) with the uploaded file.
#
# Steps (in recover mode):
#   1. Overwrite the current remote file (`REMOTE_FILE`) with the backup (`REMOTE_BACKUP_FILE`).
#
# Prerequisites:
#   1. `sshpass` is installed locally.
#   2. Make sure the current bash file has execution privileges: `chmod +x scripts/patchr.sh`
#   3. Configure variables in `scripts/AAA/config/server.sh`:
#        - REMOTE_HOST
#        - REMOTE_USER
#        - REMOTE_SSH_PORT (default: 22)
#        - REMOTE_PWD
#   4. [Optional] Create a file `~/examples/example-patch-remote.txt` on your remote server for testing purposes.
#
# Examples (run directly for easy start, using default settings):
#   * `./scripts/patchr.sh`
#
#        Patch remote file `~/examples/example-patch-remote.txt` with the local file `scripts/AAA/assets/example-patch.txt`
#   * `./scripts/patchr.sh recover`
#
#        Recover from backup
#
# Usage:
#   * `./scripts/patchr.sh`
#
#        Patch remote file
#   * `./scripts/patchr.sh recover`
#
#        Recover from backup
#
# Script Options (variables inside this script):
#   * LOCAL_FILE         : Path to the local file that will be uploaded.
#   * REMOTE_UPLOAD_FILE : Remote file path where the local file will be uploaded.
#   * REMOTE_FILE        : The original remote file to be backed up before overwriting.
#   * REMOTE_BACKUP_FILE : Path where the backup of REMOTE_FILE will be stored.

#   * USE_RSYNC          : (true/false) If true, use 'rsync' for uploading; otherwise use 'scp'.
#   * SILENT             : (true/false) If true, suppresses all confirmation prompts (auto-approve).
#   * USE_SUDO           : (true/false) If true, these commands will be executed with sudo privileges on the remote machine.  
#        Reminder: With sudo, `~` points to `/root`, not the current user's home.
#
# Global Env:
#   * ROOT : The absolute path of scripts directory.
#
# Author: Craig Brown
# Since: 1.1.0
# Date: April 16, 2025
# ************************************************************************************
source "$(dirname "${BASH_SOURCE[0]}")/AAA/config/global.sh"

# ================================================================== Required Configurations
source "$ROOT/AAA/config/server.sh"

# Upload the LOCAL_FILE to the REMOTE_UPLOAD_FILE
LOCAL_FILE="$ROOT/AAA/assets/example-patch.txt"
REMOTE_UPLOAD_FILE='~/tmp/example-patch.txt.upload'

# Copy the REMOTE_FILE to REMOTE_BACKUP_FILE before overwriting
REMOTE_FILE='~/examples/example-patch-remote.txt'
REMOTE_BACKUP_FILE='~/tmp/example-patch-remote.txt.bak'

# ================================================================== Optional Configurations
SILENT=false
USE_RSYNC=false
USE_SUDO=false

# ================================================================== Functions
source "$ROOT/AAA/common/functions.sh"
source "$ROOT/AAA/common/upload.sh"
set -e

# ================================================================== Logic
check_ssh_connection
trust_host
check_sshpass_installed

resolved_remote_upload_file=$(resolve_remote_path "$REMOTE_UPLOAD_FILE")
resolved_remote_file=$(resolve_remote_path "$REMOTE_FILE")
# ================================================================== Recovery Mode
if [ "$1" == "recover" ]; then
  ask $SILENT "Do you want to restore the remote file '$REMOTE_FILE' from the backup file '$REMOTE_BACKUP_FILE'? (y/n): "
  remote_execute "cp $REMOTE_BACKUP_FILE $REMOTE_FILE -f"
  echo "[INFO] Recovery completed. Remote file info:"
  remote_execute "ls -al $REMOTE_FILE"
  exit 0
fi

# ================================================================== 1. Upload File
echo "==================================== Upload the local file"
echo "[INFO] Local file info:"
ls -al "$LOCAL_FILE"

if remote_execute "[ -f \"$REMOTE_UPLOAD_FILE\" ]"; then
  ask $SILENT "The remote file '$REMOTE_UPLOAD_FILE' already exists. Do you want to overwrite it with '$LOCAL_FILE'? (y/n): "
else
  ask $SILENT "Do you want to upload '$LOCAL_FILE' to '$REMOTE_UPLOAD_FILE'? (y/n): "
fi

upload_file_to_file "$LOCAL_FILE" "$resolved_remote_upload_file" $USE_RSYNC $SILENT
echo
echo "[INFO] Upload completed. Printing uploaded file info:"
remote_execute "ls -al $REMOTE_UPLOAD_FILE"
#ask $SILENT "Is the local file successfully uploaded? (y/n): "
echo

# ================================================================== 2. Backup File
echo "==================================== Backup the server file"
if ! remote_execute "[ -f \"$resolved_remote_file\" ]"; then
  echo "[ERROR] Remote file does not exist: $REMOTE_FILE, Aborting script."
  exit 1
fi

echo "[INFO] Remote file info before backup:"
remote_execute "ls -al $REMOTE_FILE"
ask $SILENT "Do you want to back up the remote file '$REMOTE_FILE' to '$REMOTE_BACKUP_FILE'? (y/n): "
remote_execute "cp $REMOTE_FILE $REMOTE_BACKUP_FILE -f"
echo
echo "[INFO] Backup completed. Printing backup file info:"
remote_execute "ls -al $REMOTE_BACKUP_FILE"
# ask $SILENT "Is the backup file created successfully? (y/n): "
echo

# ================================================================== 3. Overwrite File
echo "==================================== Overwrite the server file"
remote_execute "ls -al $REMOTE_UPLOAD_FILE"
ask $SILENT "Do you want to overwrite the remote file '$REMOTE_FILE' with the uploaded file '$REMOTE_UPLOAD_FILE'? (y/n): "
remote_execute "cp $REMOTE_UPLOAD_FILE $REMOTE_FILE -f"
echo
echo "[INFO] Overwrite completed. Final file info:"
remote_execute "ls -al $REMOTE_FILE"
