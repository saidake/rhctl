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
REM Install and run the MailHog Docker image.
REM
REM Prerequisites:
REM   1. **Docker Desktop** is installed and running locally. 
REM      (see [Docker Desktop / Installing on Local Windows](#installing-on-local-windows)).
REM 
REM Since: 1.0.0
REM Date: October 16, 2025
REM ************************************************************************************

echo Checking Docker...
docker --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Docker is not installed or not in PATH.
    exit /b 1
)

echo Checking for existing MailHog image...
docker images mailhog/mailhog:latest --format "{{.Repository}}:{{.Tag}}" | findstr /I "mailhog/mailhog:latest" >nul
if %errorlevel%==0 (
    echo [INFO] Existing MailHog container found. Removing...
    docker rm -f mailhog >nul 2>&1
) else (
    echo [INFO] Pulling MailHog image...
    docker pull mailhog/mailhog:latest
    if errorlevel 1 (
        echo [ERROR] Failed to pull MailHog image.
        exit /b 1
    )
)

echo Starting MailHog container...
docker run -d ^
    --name mailhog ^
    -p 1025:1025 ^
    -p 8025:8025 ^
    mailhog/mailhog
REM SMTP port： 1025
REM Web UI port： 8025

if errorlevel 1 (
    echo [ERROR] Failed to start MailHog container.
    exit /b 1
)

echo.
echo MailHog is running!
echo SMTP server: localhost:1025
echo Web UI: http://localhost:8025
echo Use SMTP server in your app to test sending emails.
