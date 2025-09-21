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

[[ -n "$_UPLOAD_SH_INCLUDED" ]] && return
_UPLOAD_SH_INCLUDED=1

source "$ROOT/AAA/common/functions.sh"

generate_random_temp_path() {
  local filename="$1"
  local random_suffix
  random_suffix=$(date +%s%N | sha256sum | head -c 8)
  echo "$SANDBOX_TEMP_DIR/${filename}.${random_suffix}"
}

ensure_remote_temp_dir() {
  # if ! remote_execute "[ -d '$SANDBOX_TEMP_DIR' ]"; then
    # echo "[INFO] Creating temp directory '$SANDBOX_TEMP_DIR' on remote host..."
    remote_execute "mkdir -p '$SANDBOX_TEMP_DIR' && chmod 777 '$SANDBOX_TEMP_DIR'"
  # fi
}

upload_with_tool() {
  local local_path="$1"
  local remote_path="$2"

  if [ "$USE_RSYNC" = true ]; then
    sshpass -p "$REMOTE_PWD" rsync -avz -e "ssh -q -p $REMOTE_SSH_PORT" "$local_path" "$REMOTE_USER@$REMOTE_HOST:$remote_path"
  else
    sshpass -p "$REMOTE_PWD" scp -P "$REMOTE_SSH_PORT" "$local_path" "$REMOTE_USER@$REMOTE_HOST:$remote_path"
  fi
}

upload_with_sudo_move() {
  local local_path="$1"
  local remote_path="$2"
  # echo "[DEBUG] upload_with_sudo_move - remote_path: $remote_path" | cat -A

  # Check if the target path is inside the SANDBOX_TEMP_DIR
  if [[ "$remote_path" == "$SANDBOX_TEMP_DIR"* ]]; then
    # echo "[INFO] Target is inside SANDBOX_TEMP_DIR, directly uploading to: $remote_path"
    
    # Ensure directory exists and is world-writable
    ensure_remote_temp_dir

    upload_with_tool "$local_path" "$remote_path"
    return
  fi

  # If not in temp dir, use intermediate path and mv
  local filename
  filename=$(basename "$remote_path")

  local temp_path
  temp_path=$(generate_random_temp_path "$filename")
  # echo "[DEBUG] upload_with_sudo_move - temp_path: $temp_path" | cat -A

  ensure_remote_temp_dir

  upload_with_tool "$local_path" "$temp_path"
  remote_execute "mv '$temp_path' '$remote_path'"
}



upload_file_to_dir() {
  local source="$1"
  local absolute_remote_dir="$2"
  local filename
  filename=$(basename "$source")
  local destination="$absolute_remote_dir/$filename"

  echo "[INFO] Uploading file '$source' to remote directory '$absolute_remote_dir'..."

  if [ "$USE_SUDO" = true ]; then
    upload_with_sudo_move "$source" "$destination"
  else
    upload_with_tool "$source" "$destination"
  fi

  echo "[INFO] Upload complete."
}


upload_file_to_file() {
  local local_file="$1"
  local remote_file="$2"
  local use_rsync="${3:-false}"
  local silent="${4:-false}"

  if [[ ! -f "$local_file" ]]; then
    echo "[ERROR] Local file '$local_file' does not exist or is not a regular file." >&2
    return 1
  fi

  local absolute_remote_file
  absolute_remote_file="$(resolve_remote_path "$remote_file")"
  local remote_dir
  remote_dir="$(dirname "$absolute_remote_file")"

  if remote_file_exists "$absolute_remote_file"; then
    ask "$silent" "[WARN] Remote file '$remote_file' already exists. Overwrite? (y/n): "
  fi

  if ! remote_dir_exists "$remote_dir"; then
    echo "[INFO] Remote directory '$remote_dir' does not exist. Creating it."
    remote_execute "mkdir -p \"$remote_dir\""
  fi

  echo "[INFO] Uploading file '$local_file' to remote path '$remote_file'..."


  if [ "$USE_SUDO" = true ]; then
    upload_with_sudo_move "$local_file" "$absolute_remote_file"
  else
    upload_with_tool "$local_file" "$absolute_remote_file"
  fi

  echo "[INFO] Successfully uploaded '$local_file' to '$remote_file'."
}


upload_file_or_dir_to_dir() {
  local source="$1"
  local remote_dir="$2"
  local use_rsync="$3"
  local silent="$4"

  # echo "upload_file_or_dir_to_dir source: $source" | cat -A
  absolute_remote_dir=$(resolve_remote_path "$remote_dir")
  # echo "[DEBUG] upload_file_or_dir_to_dir - absolute_remote_dir: $absolute_remote_dir" | cat -A

  if [[ ! -e "$source" ]]; then
    echo "[ERROR] Source '$source' does not exist." >&2
    exit 1
  fi
  if remote_file_exists "$absolute_remote_dir"; then
    echo "[ERROR] Remote path '$remote_dir' is a file, not a directory." >&2
    exit 1
  fi

  if ! remote_dir_exists "$absolute_remote_dir"; then
    echo "[INFO] Remote directory '$remote_dir' does not exist. Creating it."
    remote_execute "mkdir -p \"$absolute_remote_dir\""
  fi

  if ! remote_execute "[[ -d \"$absolute_remote_dir\" ]]" ; then
    echo "[ERROR] Remote destination '$remote_dir' is not a directory." >&2
    exit 1
  fi

  if [[ -d "$source" ]]; then
    echo "[INFO] Uploading contents of directory '$source' to '$remote_dir' ..."
    for item in "$source"/*; do
      local base_item
      base_item=$(basename "$item")

      if remote_file_or_dir_exists "$absolute_remote_dir/$base_item"; then
        ask "$silent" "[WARN] '$base_item' already exists in '$remote_dir'. Overwrite? (y/n): "
      fi

      upload_file_to_dir "$item" "$absolute_remote_dir"

      if [[ $? -ne 0 ]]; then
        echo "[ERROR] Failed to upload '$item' to '$remote_dir'." >&2
        exit 1
      else
        echo "[INFO] Successfully uploaded '$item' to '$remote_dir'."
      fi
    done

  elif [[ -f "$source" ]]; then
    local base_file
    base_file=$(basename "$source")

    echo "[INFO] Uploading file '$source' to '$remote_dir' ..."
    if remote_file_or_dir_exists "$absolute_remote_dir/$base_file"; then
      ask "$silent" "[WARN] '$base_file' already exists in '$remote_dir'. Overwrite? (y/n): "
    fi
    upload_file_to_dir "$source" "$absolute_remote_dir"

    if [[ $? -ne 0 ]]; then
      echo "[ERROR] Failed to upload '$source' to '$remote_dir'." >&2
      exit 1
    else
      echo "[INFO] Successfully uploaded '$source' to '$remote_dir'."
    fi

  else
    echo "[ERROR] Unsupported source type: $source" >&2
    exit 1
  fi
}
