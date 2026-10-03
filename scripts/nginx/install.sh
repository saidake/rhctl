#!/bin/bash

# ****************************************************************************************************
# Copyright (C) 2022-2026 rhctl Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Installs Nginx and enables its systemd service.
#
# If exists, skip — safe to re-run.
#
# Usage:
#   ./install.sh
#
# Required Parameters:
#   (none)
#
# Optional Parameters:
#   --package <name>
#       Nginx package to install (default: `nginx`).
#         Example name values: `nginx`, `nginx-core`
#
# Override Parameters:
#   RHCTL_NGINX_PACKAGE=<name>
#       Same as `--package`.
#
# Since : 0.1.0
# Date  : Sep 29, 2026
# ****************************************************************************************************

set -e

# ========================================================================= Parameter

NGINX_PACKAGE="${RHCTL_NGINX_PACKAGE:-nginx}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --package)
            [[ $# -ge 2 ]] || {
                echo "Error: --package requires a value." >&2
                exit 1
            }
            NGINX_PACKAGE="$2"
            shift 2
            ;;
        -h|--help)
            sed -n '1,/^# \*\{4,\}/p' "$0"
            exit 0
            ;;
        *)
            echo "Error: unknown parameter: $1" >&2
            exit 1
            ;;
    esac
done

# ========================================================================= Methods

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# ========================================================================= Check Nginx

if command_exists nginx; then
    echo "Nginx is already installed: $(nginx -v 2>&1)"

    if command_exists systemctl; then
        systemctl enable nginx >/dev/null 2>&1 || true
        systemctl start nginx
    fi

    exit 0
fi

# ========================================================================= Install Nginx

if ! command_exists apt-get; then
    echo "Error: apt-get is required to install Nginx." >&2
    exit 1
fi

echo "Installing ${NGINX_PACKAGE}..."

apt-get update
apt-get install -y "$NGINX_PACKAGE"

# ========================================================================= Enable / start Nginx

if command_exists systemctl; then
    systemctl enable nginx
    systemctl start nginx
fi

# ========================================================================= Final output

echo
echo "Nginx installation completed."
echo "Version: $(nginx -v 2>&1)"
echo "Service: $(systemctl is-active nginx 2>/dev/null || echo "unknown")"