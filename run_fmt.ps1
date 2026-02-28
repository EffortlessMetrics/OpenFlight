#!/usr/bin/env pwsh
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Step 1: Running 'cargo fmt --all 2>&1'" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
cargo fmt --all 2>&1
Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "Step 2: Running 'cargo fmt --all -- --check 2>&1'" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
cargo fmt --all -- --check 2>&1
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "All steps completed!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
