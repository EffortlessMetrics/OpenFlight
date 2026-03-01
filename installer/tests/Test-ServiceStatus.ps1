# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Test-ServiceStatus.ps1 — Verify the Flight Hub Windows service can be
# started and stopped.
#
# Usage (requires Administrator):
#   .\Test-ServiceStatus.ps1

[CmdletBinding()]
param()

$ServiceName = "FlightHub"
$Pass = 0
$Fail = 0

function Assert-Ok {
    param([string]$Description, [scriptblock]$Action)
    try {
        & $Action
        Write-Host "  [OK]   $Description" -ForegroundColor Green
        $script:Pass++
    } catch {
        Write-Host "  [FAIL] $Description — $($_.Exception.Message)" -ForegroundColor Red
        $script:Fail++
    }
}

function Assert-ServiceStatus {
    param([string]$Description, [string]$Expected)
    $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
    if ($svc -and $svc.Status -eq $Expected) {
        Write-Host "  [OK]   $Description (Status=$($svc.Status))" -ForegroundColor Green
        $script:Pass++
    } else {
        $actual = if ($svc) { $svc.Status } else { "NotFound" }
        Write-Host "  [FAIL] $Description (expected=$Expected, got=$actual)" -ForegroundColor Red
        $script:Fail++
    }
}

Write-Host "=== Flight Hub Service Status Test ===" -ForegroundColor Cyan
Write-Host ""

# Check service exists
$svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
if (-not $svc) {
    Write-Host "  [FAIL] Service '$ServiceName' not registered. Install Flight Hub first." -ForegroundColor Red
    exit 1
}

Write-Host "-- Start service --"
Assert-Ok "Start-Service $ServiceName" { Start-Service -Name $ServiceName -ErrorAction Stop }
Start-Sleep -Seconds 2
Assert-ServiceStatus "service is running after start" "Running"

Write-Host ""
Write-Host "-- Stop service --"
Assert-Ok "Stop-Service $ServiceName" { Stop-Service -Name $ServiceName -ErrorAction Stop }
Start-Sleep -Seconds 2
Assert-ServiceStatus "service is stopped after stop" "Stopped"

Write-Host ""
Write-Host "=== Results: $Pass passed, $Fail failed ===" -ForegroundColor $(if ($Fail -gt 0) { "Red" } else { "Green" })

if ($Fail -gt 0) { exit 1 }
