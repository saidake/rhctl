@echo off
REM ************************************************************************************
REM Copyright (C) 2022-2026 rhctl Contributors
REM
REM This program is free software: you can redistribute it and/or modify
REM it under the terms of the GNU General Public License as published by
REM the Free Software Foundation, either version 3 of the License, or
REM (at your option) any later version.
REM
REM This program is distributed in the hope that it will be useful,
REM but WITHOUT ANY WARRANTY; without even the implied warranty of
REM MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
REM GNU General Public License for more details.
REM
REM You should have received a copy of the GNU General Public License
REM along with this program.  If not, see <https://www.gnu.org/licenses/>.
REM ************************************************************************************
REM Stop the MailHog Docker image.
REM
REM Prerequisites:
REM   1. **Docker Desktop** is installed and running locally. 
REM      (see [Docker Desktop / Installing on Local Windows](#installing-on-local-windows)).
REM 
REM Since: 1.0.0
REM Date: October 16, 2025
REM ************************************************************************************

echo [INFO] Checking if MailHog container is running...
docker ps -a --filter "name=mailhog" --format "{{.Names}}" | findstr /I "mailhog" >nul
if %errorlevel% neq 0 (
    echo [INFO] MailHog container is not running.
    exit /b 0
)

echo [INFO] Stopping MailHog container...
docker stop mailhog >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Failed to stop MailHog container.
    exit /b 1
)

echo [INFO] Removing MailHog container...
docker rm mailhog >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Failed to remove MailHog container.
    exit /b 1
)

echo [INFO] MailHog container stopped and removed successfully.