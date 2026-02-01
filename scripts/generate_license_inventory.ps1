# Flight Hub License Inventory Generator
# 
# Generates third-party-components.toml from Cargo.lock
# Requirements: 12.1, 12.2
#
# Usage: .\scripts\generate_license_inventory.ps1

$ErrorActionPreference = "Stop"

Write-Host "Generating third-party license inventory..." -ForegroundColor Cyan

# Check if cargo-license is installed
$cargoLicense = Get-Command cargo-license -ErrorAction SilentlyContinue
if (-not $cargoLicense) {
    Write-Host "Installing cargo-license..." -ForegroundColor Yellow
    cargo install cargo-license
}

# Generate license information in JSON format
Write-Host "Fetching license information from crates.io..." -ForegroundColor Yellow
$licenseJson = cargo license --json 2>$null | ConvertFrom-Json

# Filter out our own crates
$thirdParty = $licenseJson | Where-Object { -not $_.name.StartsWith("flight-") }

# Build inventory
$inventory = @{
    generated_at = (Get-Date -Format "o")
    total_dependencies = $thirdParty.Count
    components = @()
}

foreach ($dep in $thirdParty) {
    $component = @{
        name = $dep.name
        version = $dep.version
        license = if ($dep.license) { $dep.license } else { "Unknown" }
        repository = $dep.repository
    }
    $inventory.components += $component
}

# Sort by name
$inventory.components = $inventory.components | Sort-Object { $_.name }

# Convert to TOML format (simple implementation)
$tomlContent = @"
# Third-Party Components Inventory
# Generated: $($inventory.generated_at)
# Total dependencies: $($inventory.total_dependencies)

"@

foreach ($comp in $inventory.components) {
    $tomlContent += @"

[[components]]
name = "$($comp.name)"
version = "$($comp.version)"
license = "$($comp.license)"
"@
    if ($comp.repository) {
        $tomlContent += "`nrepository = `"$($comp.repository)`""
    }
}

# Write output
$tomlContent | Out-File -FilePath "third-party-components.toml" -Encoding utf8

Write-Host "`nGenerated third-party-components.toml with $($inventory.total_dependencies) dependencies" -ForegroundColor Green

# Generate markdown summary
$mdContent = @"
# Third-Party Components

Generated: $($inventory.generated_at)

Total dependencies: $($inventory.total_dependencies)

## License Summary

"@

# Group by license
$byLicense = $inventory.components | Group-Object { $_.license }
$mdContent += "| License | Count |`n"
$mdContent += "|---------|-------|`n"
foreach ($group in ($byLicense | Sort-Object { -$_.Count })) {
    $mdContent += "| $($group.Name) | $($group.Count) |`n"
}

$mdContent += "`n## All Components`n`n"
$mdContent += "| Name | Version | License |`n"
$mdContent += "|------|---------|---------|`n"
foreach ($comp in $inventory.components) {
    $mdContent += "| $($comp.name) | $($comp.version) | $($comp.license) |`n"
}

$mdContent | Out-File -FilePath "docs/reference/third-party-licenses.md" -Encoding utf8

Write-Host "Generated docs/reference/third-party-licenses.md" -ForegroundColor Green
