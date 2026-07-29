#!/bin/bash
# ************************************************************************************
# Copyright (C) 2022-2026 rsctl Contributors
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
# Apply test configuration to Redis for remote client access.
# Port: 6379
#
# Configures:
#   - bind 0.0.0.0 ::          (accept connections on all interfaces)
#   - protected-mode no        (allow remote connections without auth; test only)
#   - firewall rule for 6379   (if ufw is available)
#
# This script is idempotent and safe to run multiple times.
#
# Since : 1.0.1
# Date  : Jul 7, 2026
# ************************************************************************************

CONFIG_CHANGED=false

# Update a Redis config setting only when the current value differs.
# Returns 0 when a change was made, 1 when already correct.
update_redis_setting() {
    local key="$1"
    local value="$2"
    local conf="$3"
    local expected="${key} ${value}"
    local current_line

    current_line=$(grep -E "^[[:space:]]*${key}[[:space:]]+" "$conf" | head -1 || true)

    if [ -n "$current_line" ]; then
        local normalized_current normalized_expected
        normalized_current=$(echo "$current_line" | sed 's/^[[:space:]]*//;s/[[:space:]]\+/ /g')
        normalized_expected=$(echo "$expected" | sed 's/[[:space:]]\+/ /g')

        if [ "$normalized_current" = "$normalized_expected" ]; then
            echo "[INFO] Redis config '${key}' is already set correctly"
            return 1
        fi

        sudo sed -i "s/^[[:space:]]*${key}[[:space:]].*/${expected}/" "$conf"
        echo "[INFO] Updated Redis config '${key}' to '${value}'"
        return 0
    fi

    if grep -qE "^[[:space:]]*#[[:space:]]*${key}[[:space:]]+" "$conf"; then
        sudo sed -i "s/^[[:space:]]*#[[:space:]]*${key}[[:space:]].*/${expected}/" "$conf"
        echo "[INFO] Uncommented and set Redis config '${key}' to '${value}'"
        return 0
    fi

    echo "$expected" | sudo tee -a "$conf" > /dev/null
    echo "[INFO] Added Redis config '${key}' with value '${value}'"
    return 0
}

apply_redis_setting() {
    if update_redis_setting "$1" "$2" "$3"; then
        CONFIG_CHANGED=true
    fi
}

configure_firewall() {
    if ! command -v ufw &> /dev/null; then
        echo "[INFO] ufw not installed — skipping firewall configuration"
        return 0
    fi

    if sudo ufw status 2>/dev/null | grep -qE '(6379/tcp|6379\s)'; then
        echo "[INFO] Firewall rule for Redis port 6379 already exists"
        return 0
    fi

    echo "[INFO] Adding firewall rule to allow Redis port 6379/tcp"
    sudo ufw allow 6379/tcp comment 'Redis test server'
}

# --- Pre-checks ---

if ! command -v redis-server &> /dev/null; then
    echo "[ERROR] Redis is not installed. Run install.sh first."
    exit 1
fi

REDIS_CONF="/etc/redis/redis.conf"
if [ ! -f "$REDIS_CONF" ]; then
    echo "[ERROR] Redis config file not found at ${REDIS_CONF}"
    exit 1
fi

echo "[INFO] Applying Redis test configuration from ${REDIS_CONF}"

# --- Redis connection settings ---

apply_redis_setting "bind" "0.0.0.0 ::" "$REDIS_CONF"
apply_redis_setting "protected-mode" "no" "$REDIS_CONF"

# --- Firewall ---

configure_firewall

# --- Apply changes ---

if [ "$CONFIG_CHANGED" = true ]; then
    echo "[INFO] Restarting Redis to apply configuration changes..."
    sudo systemctl restart redis-server
else
    echo "[INFO] No Redis configuration changes needed"
    if ! systemctl is-active --quiet redis-server; then
        echo "[WARN] Redis service is not running. Starting it now..."
        sudo systemctl start redis-server
    fi
fi

# --- Verification ---

echo "[INFO] Checking Redis service status..."
if systemctl is-active --quiet redis-server; then
    echo "[INFO] Redis service is running"
else
    echo "[ERROR] Redis service failed to start"
    sudo systemctl status redis-server --no-pager || true
    exit 1
fi

echo "[INFO] Checking Redis listen address..."
if command -v ss &> /dev/null; then
    if ss -tln 2>/dev/null | grep -qE '0\.0\.0\.0:6379|\[::\]:6379|\*:6379'; then
        echo "[INFO] Redis is listening on all interfaces (port 6379)"
    else
        echo "[WARN] Redis may not be listening on all interfaces yet"
        ss -tln 2>/dev/null | grep 6379 || true
    fi
fi

echo "[INFO] Testing local Redis connectivity..."
if redis-cli ping 2>/dev/null | grep -q PONG; then
    echo "[INFO] Redis responded to ping (PONG)"
else
    echo "[ERROR] Redis did not respond to ping"
    exit 1
fi

echo "[INFO] Redis test configuration applied successfully"
echo "[INFO] Clients can connect on port 6379 from any host"
echo "[WARN] protected-mode is disabled — intended for test environments only"
