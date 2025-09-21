@echo off
REM ************************************************************************************
REM Copyright 2022-2025 the original author or authors.
REM
REM Licensed under the Apache License, Version 2.0 (the "License");
REM you may not use this file except in compliance with the License.
REM You may obtain a copy of the License at
REM
REM      https://www.apache.org/licenses/LICENSE-2.0
REM
REM Unless required by applicable law or agreed to in writing, software
REM distributed under the License is distributed on an "AS IS" BASIS,
REM WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
REM See the License for the specific language governing permissions and
REM limitations under the License.
REM ************************************************************************************
REM Stop the MailHog Docker image.
REM
REM Prerequisites:
REM   1. **Docker Desktop** is installed and running locally. 
REM      (see [Docker Desktop / Installing on Local Windows](#installing-on-local-windows)).
REM 
REM Author: Craig Brown
REM Since: 1.3.3
REM Date: August 5, 2025
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