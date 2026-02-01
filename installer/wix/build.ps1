<#
.SYNOPSIS
    Build script for Flight Hub Windows MSI installer.

.DESCRIPTION
    This script builds the Flight Hub MSI installer using WiX Toolset v4.
    It compiles the WiX source files and links them into a signed MSI package.

.PARAMETER Configuration
    Build configuration: Debug or Release. Default is Release.

.PARAMETER Version
    Version string for the installer. If not specified, reads from Cargo.toml.

.PARAMETER Sign
    Whether to sign the MSI with a code signing certificate. Default is $true for Release.

.PARAMETER CertificatePath
    Path to the code signing certificate (.pfx file).

.PARAMETER CertificatePassword
    Password for the code signing certificate.

.PARAMETER OutputDir
    Output directory for the MSI. Default is .\output.

.EXAMPLE
    .\build.ps1 -Configuration Release -Sign $true

.EXAMPLE
    .\build.ps1 -Version "1.0.0" -OutputDir "C:\builds"

.NOTES
    Requirements:
    - WiX Toolset v4 (wix.exe in PATH or WIX_PATH environment variable)
    - Rust toolchain for building binaries
    - Code signing certificate for signed builds (Requirement 10.1)
#>

[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    
    [string]$Version = "",
    
    [bool]$Sign = ($Configuration -eq "Release"),
    
    [string]$CertificatePath = "",
    
    [SecureString]$CertificatePassword = $null,
    
    [string]$OutputDir = ".\output"
)

# Strict mode for better error handling
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Script constants
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Resolve-Path (Join-Path $ScriptDir "..\..") 
$WixSourceDir = $ScriptDir
$TempDir = Join-Path $ScriptDir "temp"

# WiX source files
$WixSources = @(
    "Product.wxs",
    "Components.wxs"
)

# ============================================
# Helper Functions
# ============================================

function Write-Step {
    param([string]$Message)
    Write-Host "`n=== $Message ===" -ForegroundColor Cyan
}

function Write-Success {
    param([string]$Message)
    Write-Host "[OK] $Message" -ForegroundColor Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARN] $Message" -ForegroundColor Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor Red
}

function Get-WixPath {
    # Check for WiX in PATH
    $wixExe = Get-Command "wix" -ErrorAction SilentlyContinue
    if ($wixExe) {
        return $wixExe.Source
    }
    
    # Check WIX_PATH environment variable
    if ($env:WIX_PATH) {
        $wixExe = Join-Path $env:WIX_PATH "wix.exe"
        if (Test-Path $wixExe) {
            return $wixExe
        }
    }
    
    # Check common installation paths
    $commonPaths = @(
        "${env:ProgramFiles}\WiX Toolset v4\bin\wix.exe",
        "${env:ProgramFiles(x86)}\WiX Toolset v4\bin\wix.exe",
        "${env:USERPROFILE}\.dotnet\tools\wix.exe"
    )
    
    foreach ($path in $commonPaths) {
        if (Test-Path $path) {
            return $path
        }
    }
    
    throw "WiX Toolset v4 not found. Please install it or set WIX_PATH environment variable."
}

function Get-VersionFromCargo {
    $cargoToml = Join-Path $RootDir "Cargo.toml"
    if (-not (Test-Path $cargoToml)) {
        throw "Cargo.toml not found at $cargoToml"
    }
    
    $content = Get-Content $cargoToml -Raw
    if ($content -match 'version\s*=\s*"([^"]+)"') {
        return $matches[1]
    }
    
    throw "Could not extract version from Cargo.toml"
}

function Build-RustBinaries {
    param([string]$Config)
    
    Write-Step "Building Rust binaries ($Config)"
    
    $cargoArgs = @("build", "--workspace")
    if ($Config -eq "Release") {
        $cargoArgs += "--release"
    }
    
    Push-Location $RootDir
    try {
        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build failed with exit code $LASTEXITCODE"
        }
        Write-Success "Rust binaries built successfully"
    }
    finally {
        Pop-Location
    }
}

function Get-BinaryDir {
    param([string]$Config)
    
    $targetDir = Join-Path $RootDir "target"
    if ($Config -eq "Release") {
        return Join-Path $targetDir "release"
    }
    return Join-Path $targetDir "debug"
}

function Prepare-StagingDir {
    Write-Step "Preparing staging directory"
    
    # Clean and create temp directory
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $TempDir -Force | Out-Null
    
    # Create subdirectories
    $dirs = @("bin", "config", "docs", "lua")
    foreach ($dir in $dirs) {
        New-Item -ItemType Directory -Path (Join-Path $TempDir $dir) -Force | Out-Null
    }
    
    Write-Success "Staging directory prepared at $TempDir"
}

function Stage-Files {
    param([string]$BinDir)
    
    Write-Step "Staging files for installer"
    
    # Stage binaries
    $binaries = @("flightd.exe", "flightctl.exe")
    foreach ($binary in $binaries) {
        $src = Join-Path $BinDir $binary
        $dst = Join-Path $TempDir "bin\$binary"
        if (Test-Path $src) {
            Copy-Item $src $dst
            Write-Success "Staged $binary"
        }
        else {
            Write-Warning "Binary not found: $src"
        }
    }
    
    # Stage configuration files (create defaults if not present)
    $configDir = Join-Path $TempDir "config"
    
    # Create default config.toml
    $defaultConfig = @"
# Flight Hub Configuration
# See documentation for all available options

[general]
# Log level: trace, debug, info, warn, error
log_level = "info"

[scheduler]
# Target frequency in Hz
frequency = 250

[ffb]
# Enable force feedback
enabled = true
# Safety envelope enabled
safety_enabled = true
"@
    Set-Content -Path (Join-Path $configDir "config.toml") -Value $defaultConfig
    
    # Create default profile
    $defaultProfile = @"
# Default Flight Hub Profile
# This is a template profile - customize for your setup

[profile]
name = "Default"
description = "Default Flight Hub profile"

[axes]
# Axis mappings go here

[buttons]
# Button mappings go here
"@
    Set-Content -Path (Join-Path $configDir "default.profile.toml") -Value $defaultProfile
    
    # Create integration config directories
    $integrationConfigDir = Join-Path $configDir "integration"
    New-Item -ItemType Directory -Path $integrationConfigDir -Force | Out-Null
    
    # MSFS config
    $msfsConfig = @"
# MSFS Integration Configuration
[simconnect]
app_name = "Flight Hub"
"@
    Set-Content -Path (Join-Path $integrationConfigDir "msfs.config.toml") -Value $msfsConfig
    
    # X-Plane config
    $xplaneConfig = @"
# X-Plane Integration Configuration
[udp]
port = 49000
"@
    Set-Content -Path (Join-Path $integrationConfigDir "xplane.config.toml") -Value $xplaneConfig
    
    # DCS config
    $dcsConfig = @"
# DCS Integration Configuration
[export]
port = 7778
"@
    Set-Content -Path (Join-Path $integrationConfigDir "dcs.config.toml") -Value $dcsConfig
    
    # Stage documentation
    $docsDir = Join-Path $TempDir "docs"
    $integrationDocsDir = Join-Path $docsDir "integration"
    New-Item -ItemType Directory -Path $integrationDocsDir -Force | Out-Null
    
    # Copy docs if they exist
    $docFiles = @{
        "docs\product-posture.md" = "docs\product-posture.md"
        "docs\integration\msfs-what-we-touch.md" = "docs\integration\msfs-what-we-touch.md"
        "docs\integration\xplane-what-we-touch.md" = "docs\integration\xplane-what-we-touch.md"
        "docs\integration\dcs-what-we-touch.md" = "docs\integration\dcs-what-we-touch.md"
    }
    
    foreach ($entry in $docFiles.GetEnumerator()) {
        $src = Join-Path $RootDir $entry.Key
        $dst = Join-Path $TempDir $entry.Value
        if (Test-Path $src) {
            $dstDir = Split-Path -Parent $dst
            if (-not (Test-Path $dstDir)) {
                New-Item -ItemType Directory -Path $dstDir -Force | Out-Null
            }
            Copy-Item $src $dst
            Write-Success "Staged $($entry.Key)"
        }
        else {
            # Create placeholder
            $dstDir = Split-Path -Parent $dst
            if (-not (Test-Path $dstDir)) {
                New-Item -ItemType Directory -Path $dstDir -Force | Out-Null
            }
            Set-Content -Path $dst -Value "# Placeholder - documentation to be added"
            Write-Warning "Created placeholder for $($entry.Key)"
        }
    }
    
    # Stage Lua files
    $luaDir = Join-Path $TempDir "lua"
    $flightHubExportLua = @"
-- FlightHubExport.lua
-- Flight Hub DCS World Integration
-- 
-- This file is installed by Flight Hub and provides telemetry export
-- to the Flight Hub service.

local FlightHub = {}

FlightHub.host = "127.0.0.1"
FlightHub.port = 7778

local socket = require("socket")
local udp = socket.udp()

function FlightHub.Start()
    udp:setpeername(FlightHub.host, FlightHub.port)
end

function FlightHub.Send(data)
    if udp then
        udp:send(data)
    end
end

function FlightHub.Stop()
    if udp then
        udp:close()
    end
end

-- Hook into DCS export functions
local _prevLuaExportStart = LuaExportStart
local _prevLuaExportStop = LuaExportStop
local _prevLuaExportAfterNextFrame = LuaExportAfterNextFrame

LuaExportStart = function()
    FlightHub.Start()
    if _prevLuaExportStart then
        _prevLuaExportStart()
    end
end

LuaExportStop = function()
    FlightHub.Stop()
    if _prevLuaExportStop then
        _prevLuaExportStop()
    end
end

LuaExportAfterNextFrame = function()
    -- Export telemetry data
    local data = LoGetSelfData()
    if data then
        FlightHub.Send(net.lua2json(data))
    end
    
    if _prevLuaExportAfterNextFrame then
        _prevLuaExportAfterNextFrame()
    end
end

return FlightHub
"@
    Set-Content -Path (Join-Path $luaDir "FlightHubExport.lua") -Value $flightHubExportLua
    Write-Success "Created FlightHubExport.lua"
    
    Write-Success "File staging complete"
}

function Build-Msi {
    param(
        [string]$WixExe,
        [string]$Version,
        [string]$BinDir
    )
    
    Write-Step "Building MSI package"
    
    # Ensure output directory exists
    if (-not (Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    }
    
    $msiName = "FlightHub-$Version.msi"
    $msiPath = Join-Path $OutputDir $msiName
    
    # Build WiX variables
    $wixVars = @(
        "-d", "BinDir=$TempDir\bin",
        "-d", "ConfigDir=$TempDir\config",
        "-d", "DocsDir=$TempDir\docs",
        "-d", "LuaDir=$TempDir\lua",
        "-d", "RootDir=$RootDir",
        "-d", "Version=$Version"
    )
    
    # Compile and link in one step with WiX v4
    $wixArgs = @("build") + $wixVars
    
    foreach ($source in $WixSources) {
        $wixArgs += (Join-Path $WixSourceDir $source)
    }
    
    $wixArgs += @("-o", $msiPath)
    
    Write-Host "Running: wix $($wixArgs -join ' ')"
    
    & $WixExe @wixArgs
    if ($LASTEXITCODE -ne 0) {
        throw "WiX build failed with exit code $LASTEXITCODE"
    }
    
    Write-Success "MSI built: $msiPath"
    return $msiPath
}

function Sign-Msi {
    param(
        [string]$MsiPath,
        [string]$CertPath,
        [SecureString]$CertPassword
    )
    
    Write-Step "Signing MSI package"
    
    if (-not $CertPath) {
        Write-Warning "No certificate path provided, skipping signing"
        return
    }
    
    if (-not (Test-Path $CertPath)) {
        throw "Certificate not found: $CertPath"
    }
    
    # Find signtool
    $signtool = Get-Command "signtool" -ErrorAction SilentlyContinue
    if (-not $signtool) {
        # Check Windows SDK paths
        $sdkPaths = @(
            "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe",
            "${env:ProgramFiles}\Windows Kits\10\bin\*\x64\signtool.exe"
        )
        
        foreach ($pattern in $sdkPaths) {
            $found = Get-ChildItem $pattern -ErrorAction SilentlyContinue | Sort-Object -Descending | Select-Object -First 1
            if ($found) {
                $signtool = $found.FullName
                break
            }
        }
    }
    else {
        $signtool = $signtool.Source
    }
    
    if (-not $signtool) {
        throw "signtool.exe not found. Please install Windows SDK."
    }
    
    # Build signtool arguments
    $signArgs = @(
        "sign",
        "/f", $CertPath,
        "/fd", "SHA256",
        "/tr", "http://timestamp.digicert.com",
        "/td", "SHA256",
        "/d", "Flight Hub",
        "/du", "https://github.com/openflight/flight-hub"
    )
    
    if ($CertPassword) {
        $plainPassword = [Runtime.InteropServices.Marshal]::PtrToStringAuto(
            [Runtime.InteropServices.Marshal]::SecureStringToBSTR($CertPassword)
        )
        $signArgs += @("/p", $plainPassword)
    }
    
    $signArgs += $MsiPath
    
    & $signtool @signArgs
    if ($LASTEXITCODE -ne 0) {
        throw "Code signing failed with exit code $LASTEXITCODE"
    }
    
    Write-Success "MSI signed successfully"
}

function Cleanup {
    Write-Step "Cleaning up"
    
    if (Test-Path $TempDir) {
        Remove-Item $TempDir -Recurse -Force
        Write-Success "Removed staging directory"
    }
}

# ============================================
# Main Build Process
# ============================================

try {
    Write-Host @"

╔═══════════════════════════════════════════════════════════════╗
║           Flight Hub Windows Installer Build Script           ║
╚═══════════════════════════════════════════════════════════════╝

"@ -ForegroundColor Magenta

    # Validate WiX installation
    $wixExe = Get-WixPath
    Write-Success "Found WiX at: $wixExe"
    
    # Get version
    if (-not $Version) {
        $Version = Get-VersionFromCargo
    }
    Write-Host "Building version: $Version" -ForegroundColor White
    
    # Build Rust binaries
    Build-RustBinaries -Config $Configuration
    
    # Get binary directory
    $binDir = Get-BinaryDir -Config $Configuration
    Write-Host "Binary directory: $binDir" -ForegroundColor White
    
    # Prepare staging
    Prepare-StagingDir
    
    # Stage files
    Stage-Files -BinDir $binDir
    
    # Build MSI
    $msiPath = Build-Msi -WixExe $wixExe -Version $Version -BinDir $binDir
    
    # Sign if requested
    if ($Sign) {
        Sign-Msi -MsiPath $msiPath -CertPath $CertificatePath -CertPassword $CertificatePassword
    }
    
    # Cleanup
    Cleanup
    
    Write-Host @"

╔═══════════════════════════════════════════════════════════════╗
║                    Build Complete!                            ║
╚═══════════════════════════════════════════════════════════════╝

MSI Location: $msiPath
Version: $Version
Signed: $Sign

"@ -ForegroundColor Green

}
catch {
    Write-Error $_.Exception.Message
    Write-Host $_.ScriptStackTrace -ForegroundColor Red
    exit 1
}
