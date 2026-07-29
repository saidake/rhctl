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
# Install PostgreSQL.
# Default Port: 5432
# Commands:
#   systemctl start postgresql
#
# Since : 1.0.1
# Date  : Feb 8, 2026
# ************************************************************************************

# Check if PostgreSQL is already installed
if command -v psql &> /dev/null; then
    PG_VERSION=$(psql --version 2>&1 | grep -oP '(?<=psql \(PostgreSQL\) )[\d.]+' | head -1)
    echo "[INFO] PostgreSQL is already installed (version: $PG_VERSION)"
    echo "[INFO] Skipping reinstallation"
    
    # Check if PostgreSQL service is running
    if systemctl is-active --quiet postgresql; then
        echo "[INFO] PostgreSQL service is currently running"
    else
        echo "[WARN] PostgreSQL service is not running. Starting it now..."
        sudo systemctl start postgresql
    fi
    
    echo "[INFO] To test PostgreSQL, run: sudo -u postgres psql -c 'SELECT version();'"
    exit 0
fi

echo "[INFO] PostgreSQL not found. Proceeding with installation..."

echo "[INFO] Install prerequisites"

sudo apt update
sudo apt install -y lsb-release curl gpg ca-certificates

echo "[INFO] Add the PostgreSQL repository key"

# Create the file repository configuration
sudo sh -c 'echo "deb http://apt.postgresql.org/pub/repos/apt $(lsb_release -cs)-pgdg main" > /etc/apt/sources.list.d/pgdg.list'

# Import the repository signing key
curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc | sudo gpg --dearmor -o /etc/apt/trusted.gpg.d/postgresql.gpg

echo "[INFO] Update package list"

sudo apt update

echo "[INFO] Install PostgreSQL (latest version)"

sudo apt install -y postgresql postgresql-contrib

echo "[INFO] Enable PostgreSQL to start on boot"

sudo systemctl enable postgresql

echo "[INFO] Start PostgreSQL service"

sudo systemctl start postgresql

echo "[INFO] Check PostgreSQL status"

sudo systemctl status postgresql --no-pager

echo "[INFO] PostgreSQL installation completed successfully"
echo "[INFO] Installed PostgreSQL version: $(psql --version)"
