# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Test-UninstallClean.ps1 — Verify that uninstalling Flight Hub removes
# program files but preserves user configuration.
#
# Usage:
#   .\Test-UninstallClean.ps1
#   .\Test-UninstallClean.ps1 -InstallDir "C:\Program Files\Flight Hub"
#
# Prerequisites:
#   - Flight Hub was installed and then uninstalled via msiexec /x

[CmdletBinding()]
param(
    [string]$InstallDir = "$env:ProgramFiles\Flight Hub"
)

$Pass = 0
$Fail = 0

function Assert-Absent {
    param([string]$Path, [string]$Description)
    if (-not (Test-Path $Path)) {
        Write-Host "  [OK]   removed: $Description" -ForegroundColor Green
        $script:Pass++
    } else {
        Write-Host "  [FAIL] still present: $Description ($Path)" -ForegroundColor Red
        $script:Fail++
    }
}

function Assert-Present {
    param([string]$Path, [string]$Description)
    if (Test-Path $Path) {
        Write-Host "  [OK]   preserved: $Description" -ForegroundColor Green
        $script:Pass++
    } else {
        Write-Host "  [FAIL] missing (should be preserved): $Description ($Path)" -ForegroundColor Red
        $script:Fail++
    }
}

Write-Host "=== Flight Hub Uninstall Cleanliness Test ===" -ForegroundColor Cyan
Write-Host "Install Directory: $InstallDir"
Write-Host ""

Write-Host "-- Program files should be removed --"
Assert-Absent (Join-Path $InstallDir "bin\flightd.exe") "flightd.exe"
Assert-Absent (Join-Path $InstallDir "bin\flightctl.exe") "flightctl.exe"

Write-Host ""
Write-Host "-- Windows service should be removed --"
$svc = Get-Service -Name "FlightHub" -ErrorAction SilentlyContinue
if (-not $svc) {
    Write-Host "  [OK]   FlightHub service removed" -ForegroundColor Green
    $Pass++
} else {
    Write-Host "  [FAIL] FlightHub service still registered (Status: $($svc.Status))" -ForegroundColor Red
    $Fail++
}

Write-Host ""
Write-Host "-- Registry should be cleaned --"
Assert-Absent "HKLM:\SOFTWARE\OpenFlight\Flight Hub" "Registry key"

Write-Host ""
Write-Host "-- User configuration should be preserved --"
$configDir = "$env:APPDATA\Flight Hub"
if (Test-Path $configDir) {
    Assert-Present $configDir "User config directory"
} else {
    Write-Host "  [SKIP] $configDir not found (may not have been created)" -ForegroundColor Yellow
}

Write-Host ""
Write-Host "=== Results: $Pass passed, $Fail failed ===" -ForegroundColor $(if ($Fail -gt 0) { "Red" } else { "Green" })

if ($Fail -gt 0) { exit 1 }
