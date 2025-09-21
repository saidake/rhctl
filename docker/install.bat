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
REM Install Docker Desktop locally on your Windows machine.
REM
REM Prerequisites:
REM   1. Open the Command Prompt with administrator privileges and navigate to the project root directory.
REM 
REM Author: Craig Brown
REM Since: 1.3.3
REM Date: August 5, 2025
REM ************************************************************************************

@echo off
setlocal

REM Function: Ensure Chocolatey is available, install if missing
:ensure_chocolatey
    REM Step 1: Check if choco command exists in PATH
    where choco >nul 2>&1
    if %errorlevel% equ 0 (
        echo [INFO] Chocolatey is already available in PATH.
        set "CHOCO_CMD=choco"
        goto :choco_done
    )

    REM Step 2: If folder exists, use absolute path to choco.exe
    if exist "C:\ProgramData\chocolatey\bin\choco.exe" (
        echo [INFO] Using existing Chocolatey at C:\ProgramData\chocolatey\bin\choco.exe
        set "CHOCO_CMD=C:\ProgramData\chocolatey\bin\choco.exe"
        goto :choco_done
    )

    REM Step 3: Install Chocolatey
    echo [INFO] Installing Chocolatey...
    powershell -NoProfile -ExecutionPolicy Bypass -Command ^
     "Set-ExecutionPolicy Bypass -Scope Process -Force; iex ((New-Object System.Net.WebClient).DownloadString('https://chocolatey.org/install.ps1'))"

    REM Step 4: Verify installation
    if exist "C:\ProgramData\chocolatey\bin\choco.exe" (
        set "CHOCO_CMD=C:\ProgramData\chocolatey\bin\choco.exe"
        echo [INFO] Chocolatey installed successfully.
    ) else (
        echo [ERROR] Chocolatey installation failed - choco.exe not found.
        exit /b 1
    )

REM Main script starts here
call :ensure_chocolatey
:choco_done

echo [INFO] Installing Docker Desktop...
%CHOCO_CMD% install docker-desktop -y

if %errorlevel% neq 0 (
    echo [ERROR] Docker Desktop installation failed.
    exit /b 1
)

echo [SUCCESS] Docker Desktop installation completed.

endlocal
pause
