@echo off
REM ************************************************************************************
REM Copyright (C) 2022-2026 rhctl Contributors
REM
REM SPDX-License-Identifier: Apache-2.0
REM ************************************************************************************
REM Install Docker Desktop locally on your Windows machine.
REM
REM Prerequisites:
REM   1. Open the Command Prompt with administrator privileges and navigate to the project root directory.
REM 
REM Since: 1.0.0
REM Date: October 16, 2025
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
