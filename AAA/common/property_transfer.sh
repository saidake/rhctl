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
# Date: July 19, 2025
# ************************************************************************************

[[ -n "$_PROPERTY_TRANSFER_SH_INCLUDED" ]] && return
_PROPERTY_TRANSFER_SH_INCLUDED=1
source "$ROOT/AAA/common/functions.sh"
source "$ROOT/AAA/common/upload.sh"

load_properties() {
    local properties_file="$1"
    local -n _file_mappings="$2"

    if [[ ! -f "$properties_file" ]]; then
        echo "[ERROR] Mapping file not found: '$properties_file'"
        exit 1
    fi

    # Detect and convert Windows line endings (\r\n) to Unix (\n)
    if grep -q $'\r' "$properties_file"; then
        echo "[INFO] Windows-style line endings detected. Converting to LF..."
        sed -i 's/\r$//' "$properties_file"
    fi

    # Ensure the file ends with a newline (LF)
    if [[ -n $(tail -c1 "$properties_file") ]]; then
        echo "[INFO] File missing final newline. Appending LF..."
        echo >> "$properties_file"
    fi

    # Parse key-value pairs and trim whitespace
    while IFS='=' read -r local_path target_path; do
        local_path=$(echo "$local_path" | xargs)
        target_path=$(echo "$target_path" | xargs)
        [[ -n "$local_path" && -n "$target_path" ]] && _file_mappings["$local_path"]="$target_path"
    done < "$properties_file"
}


upload_files_by_mappings() {
    local assets_root="$1"
    # shellcheck disable=SC2178
    local -n _file_mappings="$2"
    local use_rsync="$3"
    local silent="$4"
    echo "[INFO] Starting upload process..."
    for item in "${!_file_mappings[@]}"; do
        local_path="$assets_root/$item"
        remote_dir="${_file_mappings[$item]}"
        #echo "local_path: $local_path"
        #echo "remote_dir: $remote_dir"
        if [[ -e "$local_path" ]]; then
            upload_file_or_dir_to_dir "$local_path" "$remote_dir" "$use_rsync" "$silent"
        else
            echo "[WARN] '$item' not found in assets. Skipping."
        fi
    done
    echo "[INFO] Upload complete."
}

upload_files_by_properties() {
    local properties_file="$1"
    local assets_root="$2"
    local use_rsync="$3"
    local silent="$4"
    # shellcheck disable=SC2034
    declare -A file_mappings
    load_properties "$properties_file" file_mappings
    upload_files_by_mappings "$assets_root" file_mappings "$use_rsync" "$silent"
}