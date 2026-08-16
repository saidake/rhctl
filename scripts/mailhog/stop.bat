@echo off
REM ************************************************************************************
REM Copyright (C) 2022-2026 rhctl Contributors
REM
REM SPDX-License-Identifier: Apache-2.0
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