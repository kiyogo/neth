<#
.SYNOPSIS
    Neth Server Installation Script for Windows (Windows 10, 11, Windows Server)
.DESCRIPTION
    Automates the installation, configuration, and setup of Neth Server on Windows.
    Developed and maintained by KIYOVN (https://kiyomc.com).
#>

[CmdletBinding()]
param(
    [string]$InstallDir = "C:\neth",
    [string]$Version = "latest",
    [switch]$InstallService,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host "         Neth Server Installer - Windows Edition          " -ForegroundColor Cyan
Write-Host "           High-Performance Minecraft Engine             " -ForegroundColor White
Write-Host "             KIYOVN | https://kiyomc.com                  " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Architecture Detection
$arch = if ([System.Environment]::Is64BitOperatingSystem) {
    if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x86_64" }
} else {
    Write-Error "Neth requires a 64-bit operating system (x86_64 or ARM64)."
    exit 1
}

Write-Host "[1/6] Detected System Architecture: Windows $arch" -ForegroundColor Yellow

# Fallback for non-admin installation directory
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin -and $InstallDir -eq "C:\neth") {
    $InstallDir = Join-Path $env:USERPROFILE "neth"
    Write-Host "      Notice: Non-administrator user detected. Setting install path to: $InstallDir" -ForegroundColor DarkGray
}

# 2. Directory Structure Creation
Write-Host "[2/6] Preparing directory tree: $InstallDir" -ForegroundColor Yellow
$dirs = @(
    $InstallDir,
    (Join-Path $InstallDir "bin"),
    (Join-Path $InstallDir "world"),
    (Join-Path $InstallDir "logs"),
    (Join-Path $InstallDir "plugins")
)
foreach ($dir in $dirs) {
    if (-not (Test-Path $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
}

# 3. Binary Placement
$targetBinary = Join-Path $InstallDir "bin\neth.exe"
$localBuild = Join-Path $PSScriptRoot "..\target\release\neth.exe"
$localDebug = Join-Path $PSScriptRoot "..\target\debug\neth.exe"

Write-Host "[3/6] Setting up neth.exe binary..." -ForegroundColor Yellow
if (Test-Path $localBuild) {
    Write-Host "      Using locally compiled release binary: $localBuild" -ForegroundColor Green
    Copy-Item -Path $localBuild -Destination $targetBinary -Force
} elseif (Test-Path $localDebug) {
    Write-Host "      Using locally compiled debug binary: $localDebug" -ForegroundColor Green
    Copy-Item -Path $localDebug -Destination $targetBinary -Force
} else {
    $downloadUrl = "https://github.com/kiyogo/neth/releases/latest/download/neth-windows-$arch.exe"
    Write-Host "      Downloading latest binary from: $downloadUrl" -ForegroundColor Cyan
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $targetBinary -UseBasicParsing
    } catch {
        Write-Warning "Could not download pre-built binary. Building from source via Cargo..."
        if (Get-Command cargo -ErrorAction SilentlyContinue) {
            Push-Location (Join-Path $PSScriptRoot "..")
            cargo build --release --bin neth
            Pop-Location
            Copy-Item -Path $localBuild -Destination $targetBinary -Force
        } else {
            Write-Error "Please install Rust (cargo) or ensure internet access to download neth.exe."
            exit 1
        }
    }
}

# 4. Configuration Setup
Write-Host "[4/6] Initializing default configuration..." -ForegroundColor Yellow
$configPath = Join-Path $InstallDir "neth.toml"
if (-not (Test-Path $configPath)) {
    $sourceConfig = Join-Path $PSScriptRoot "..\neth.toml"
    if (Test-Path $sourceConfig) {
        Copy-Item -Path $sourceConfig -Destination $configPath -Force
    } else {
        @"
# Neth Server Configuration
# Website: https://kiyomc.com
server_name = "Neth High-Performance Server"
motd = "A Minecraft Server Powered by Neth (Rust)"

[network]
address = "0.0.0.0:25565"
online_mode = true
compression_threshold = 256

[bedrock]
enabled = true
address = "0.0.0.0:19132"
"@ | Set-Content -Path $configPath -Encoding UTF8
    }
}

# 5. Startup Script
Write-Host "[5/6] Creating launcher script..." -ForegroundColor Yellow
$runBat = Join-Path $InstallDir "run.bat"
@"
@echo off
title Neth Server - kiyomc.com
cd /d "%~dp0"
"%~dp0bin\neth.exe"
pause
"@ | Set-Content -Path $runBat -Encoding ASCII

# 6. Firewall & Service (if Administrator)
Write-Host "[6/6] Finalizing installation..." -ForegroundColor Yellow
if ($isAdmin) {
    try {
        New-NetFirewallRule -DisplayName "Neth Server (Java 25565)" -Direction Inbound -Protocol TCP -LocalPort 25565 -Action Allow -ErrorAction SilentlyContinue | Out-Null
        New-NetFirewallRule -DisplayName "Neth Server (Bedrock 19132)" -Direction Inbound -Protocol UDP -LocalPort 19132 -Action Allow -ErrorAction SilentlyContinue | Out-Null
        Write-Host "      Windows Firewall inbound rules added for ports 25565 (TCP) and 19132 (UDP)." -ForegroundColor Green
    } catch {
        Write-Warning "Could not configure firewall rules automatically."
    }
}

Write-Host ""
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "            Neth Server Installed Successfully!           " -ForegroundColor Green
Write-Host "==========================================================" -ForegroundColor Green
Write-Host "Installation Directory: $InstallDir" -ForegroundColor White
Write-Host "Start Server:           $runBat" -ForegroundColor Cyan
Write-Host "Configuration File:     $configPath" -ForegroundColor White
Write-Host "Documentation:          https://kiyomc.com" -ForegroundColor Cyan
Write-Host ""
