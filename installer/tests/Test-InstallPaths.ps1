# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Test-InstallPaths.ps1 — Verify expected files exist after a Flight Hub
# Windows installation.
#
# Usage:
#   .\Test-InstallPaths.ps1                              # auto-detect from registry
#   .\Test-InstallPaths.ps1 -InstallDir "C:\Program Files\Flight Hub"

[CmdletBinding()]
param(
    [string]$InstallDir = ""
)

$Pass = 0
$Fail = 0

function Test-FilePath {
    param([string]$Path, [string]$Description)
    if (Test-Path $Path) {
        Write-Host "  [OK]   $Description" -ForegroundColor Green
        $script:Pass++
    } else {
        Write-Host "  [FAIL] $Description — not found ($Path)" -ForegroundColor Red
        $script:Fail++
    }
}

function Test-ServiceExists {
    param([string]$ServiceName)
    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc) {
        Write-Host "  [OK]   Service '$ServiceName' registered (Status: $($svc.Status))" -ForegroundColor Green
        $script:Pass++
    } else {
        Write-Host "  [FAIL] Service '$ServiceName' not found" -ForegroundColor Red
        $script:Fail++
    }
}

# Auto-detect install directory from registry if not provided
if (-not $InstallDir) {
    $regKey = "HKLM:\SOFTWARE\OpenFlight\Flight Hub"
    if (Test-Path $regKey) {
        $InstallDir = (Get-ItemProperty -Path $regKey -Name InstallPath -ErrorAction SilentlyContinue).InstallPath
    }
    if (-not $InstallDir) {
        $InstallDir = "$env:ProgramFiles\Flight Hub"
    }
}

Write-Host "=== Flight Hub Install Path Verification ===" -ForegroundColor Cyan
Write-Host "Install Directory: $InstallDir"
Write-Host ""

Write-Host "-- Binaries --"
Test-FilePath (Join-Path $InstallDir "bin\flightd.exe") "flightd.exe"
Test-FilePath (Join-Path $InstallDir "bin\flightctl.exe") "flightctl.exe"

Write-Host ""
Write-Host "-- Configuration --"
Test-FilePath (Join-Path $InstallDir "config\config.toml") "config.toml"
Test-FilePath (Join-Path $InstallDir "config\default.profile.toml") "default.profile.toml"

Write-Host ""
Write-Host "-- Directories --"
Test-FilePath (Join-Path $InstallDir "logs") "logs directory"

Write-Host ""
Write-Host "-- Windows Service --"
Test-ServiceExists "FlightHub"

Write-Host ""
Write-Host "-- Registry --"
$regKey = "HKLM:\SOFTWARE\OpenFlight\Flight Hub"
if (Test-Path $regKey) {
    Write-Host "  [OK]   Registry key exists: $regKey" -ForegroundColor Green
    $Pass++
} else {
    Write-Host "  [FAIL] Registry key not found: $regKey" -ForegroundColor Red
    $Fail++
}

Write-Host ""
Write-Host "=== Results: $Pass passed, $Fail failed ===" -ForegroundColor $(if ($Fail -gt 0) { "Red" } else { "Green" })

if ($Fail -gt 0) { exit 1 }
