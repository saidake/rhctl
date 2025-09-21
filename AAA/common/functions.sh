#!/bin/bash
#
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
#
# Author: Craig Brown
# Since: 1.2.0
# Date: July 17, 2025
# ************************************************************************************

[[ -n "$_FUNCTIONS_SH_INCLUDED" ]] && return
_FUNCTIONS_SH_INCLUDED=1

check_required_env_vars() {
  local message="$1"
  shift
  local vars=("$@")
  local missing_vars=()

  for var_name in "${vars[@]}"; do
    if [[ -z "${!var_name}" ]]; then
      missing_vars+=("$var_name")
    fi
  done

  if [[ ${#missing_vars[@]} -gt 0 ]]; then
    echo "[ERROR] $message" >&2
    echo "Missing environment variables: ${missing_vars[*]}" >&2
    exit 1
  fi
}

check_required_env_vars \
  "Please source config/global.sh before running this script." \
  ROOT
check_required_env_vars \
  "Please source config/server.sh to configure server properties before running this script." \
  REMOTE_HOST \
  REMOTE_SSH_PORT \
  REMOTE_USER \
  REMOTE_PWD

check_ssh_connection(){
  if ! nc -z -w5 "$REMOTE_HOST" "$REMOTE_SSH_PORT"; then
    echo "[ERROR] Cannot connect to $REMOTE_HOST on port $REMOTE_SSH_PORT. Aborting script."
    exit 3
  fi
}

trust_host() {
  local ssh_dir="$HOME/.ssh"
  local known_hosts_file="$ssh_dir/known_hosts"
  local search_entry

  # echo "[DEBUG] HOME=$HOME, SSH_DIR=$ssh_dir"
  if [ ! -d "$ssh_dir" ]; then
    echo "[INFO] ~/.ssh does not exist. Creating directory."
    mkdir -p "$ssh_dir"
    chmod 700 "$ssh_dir"
  fi

  if [ ! -f "$known_hosts_file" ]; then
    echo "[INFO] ~/.ssh/known_hosts file does not exist. Creating it."
    touch "$known_hosts_file"
    chmod 600 "$known_hosts_file"
  fi

  # Determine search_entry format for known_hosts
  if [[ "$REMOTE_SSH_PORT" == "22" ]]; then
    search_entry="$REMOTE_HOST"
  else
    search_entry="[$REMOTE_HOST]:$REMOTE_SSH_PORT"
  fi

  # Check if already trusted
  if grep -qF "$search_entry" "$known_hosts_file" 2>/dev/null; then
    echo "[INFO] Host $search_entry already trusted, skipping."
    return
  fi


  echo "[INFO] Trusting $search_entry ..."
  ssh-keygen -R "$search_entry" > /dev/null 2>&1 || true
  if ! ssh-keyscan -p "$REMOTE_SSH_PORT" "$REMOTE_HOST" >> "$known_hosts_file" 2>/dev/null; then
    echo "[ERROR] ssh-keyscan failed for $search_entry $known_hosts_file"
    exit 2
  fi
}


# Choice function to interact with the user
ask() {
  local silent="$1"
  shift
  local prompt="${1:-Are you sure? (y/n): }"

  if [[ "$silent" != true && "$silent" != false && -n "$silent" ]]; then
    echo "[ERROR] Invalid value for silent: '$silent'. Must be 'true' or 'false'." >&2
    exit 2
  fi

  if [[ "$silent" == true ]]; then
    return 0
  fi

  while true; do
    read -p "$prompt" user_choice
    case "$user_choice" in
      [Yy]) return 0 ;;
      [Nn]) echo "Aborted by user."; exit 1 ;;
      *) echo "Please enter y or n." ;;
    esac
  done
}

remote_execute() {
  # sshpass -p "$REMOTE_PWD" ssh -q -p "$REMOTE_SSH_PORT" "$REMOTE_USER@$REMOTE_HOST" "$1"
  local REMOTE_CMD="$1"
  export SSHPASS="$REMOTE_PWD"
  printf -v ESCAPED_CMD "%q" "$REMOTE_CMD"  
  if [[ "$USE_SUDO" == true ]]; then
    sshpass -e ssh -p "$REMOTE_SSH_PORT" "$REMOTE_USER@$REMOTE_HOST" \
      "echo '$REMOTE_PWD' | sudo -S -p '' bash -c $ESCAPED_CMD"
  else
    sshpass -e ssh -p "$REMOTE_SSH_PORT" "$REMOTE_USER@$REMOTE_HOST" \
      "bash -c $ESCAPED_CMD"
  fi
}

remote_execute_local_script() {
  local local_script_path="$1"
  local remote_work_path="${2:-~}"
  local use_rsync="${3:-false}"
  local silent="${4:-false}"

  if [[ ! -f "$local_script_path" ]]; then
    echo "[ERROR] Script '$local_script_path' does not exist."
    return 1
  fi

  local rand_suffix
  rand_suffix=$(date +%s%N | sha256sum | head -c 8)
  local remote_temp_bash_file="$SANDBOX_TEMP_DIR/temp_exec_${rand_suffix}.sh"

  export SSHPASS="$REMOTE_PWD"

  if [[ "$USE_SUDO" == true ]]; then
    echo "[INFO] Uploading script to '$remote_temp_bash_file' for sudo execution..."
    upload_file_to_file "$local_script_path" "$remote_temp_bash_file" "$use_rsync" "$silent"
    echo "[INFO] Executing script with sudo..."
    remote_execute "cd $remote_work_path && bash $remote_temp_bash_file"
    remote_execute "rm -f '$remote_temp_bash_file'"
  else
    # echo "[INFO] Executing script directly without sudo..."
    sshpass -e ssh -p "$REMOTE_SSH_PORT" "$REMOTE_USER@$REMOTE_HOST" \
      "cd $remote_work_path && bash -l -s" < "$local_script_path"
  fi
}


check_sshpass_installed() {
  if ! command -v sshpass >/dev/null 2>&1; then
    echo "[ERROR] sshpass is NOT installed on local machine."
    exit 2
  fi
}

resolve_remote_path() {
  local raw_path="$1"
  remote_execute "echo $raw_path"
}

remote_dir_exists() {
  remote_execute "[ -d '$1' ]"
}

remote_file_exists() {
  remote_execute "[ -f '$1' ]"
}

remote_file_or_dir_exists() {
  local filepath="$1"
  # echo "[DEBUG] remote_file_or_dir_exists - filepath: $filepath" | cat -A
  if remote_execute "test -e '$filepath'"; then
    # echo "[DEBUG] remote_file_or_dir_exists - Exists"
    return 0  # Exists
  else
    # echo "[DEBUG] remote_file_or_dir_exists - Not Exists"
    return 1  # Not exists
  fi
}