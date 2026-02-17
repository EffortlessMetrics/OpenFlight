<#
.SYNOPSIS
    Build script for Flight Hub Windows MSI installer using WiX 3.x.

.DESCRIPTION
    This script builds the Flight Hub MSI installer using WiX Toolset 3.x.
    It compiles the WiX source files using candle.exe and links them using
    light.exe into a signed MSI package.

.PARAMETER Configuration
    Build configuration: Debug or Release. Default is Release.

.PARAMETER Version
    Version string for the installer (e.g., "1.0.0").
    If not specified, reads from Cargo.toml workspace package version.

.PARAMETER OutputPath
    Output directory for the MSI file. Default is .\output.

.PARAMETER Sign
    Whether to sign the MSI with a code signing certificate.
    Default is $true for Release configuration.

.PARAMETER CertificatePath
    Path to the code signing certificate (.pfx file).

.PARAMETER CertificatePassword
    Password for the code signing certificate (SecureString).

.PARAMETER SkipBuild
    Skip building Rust binaries (use existing binaries from target directory).

.EXAMPLE
    .\build.ps1
    Build with defaults (Release, version from Cargo.toml)

.EXAMPLE
    .\build.ps1 -Version "1.2.3" -Configuration Release
    Build version 1.2.3 in Release mode

.EXAMPLE
    .\build.ps1 -Sign $true -CertificatePath ".\cert.pfx"
    Build and sign with the specified certificate

.EXAMPLE
    .\build.ps1 -Configuration Debug -SkipBuild
    Debug build using existing binaries

.NOTES
    Requirements:
    - WiX Toolset 3.x (candle.exe and light.exe in PATH or WIX environment variable)
    - Rust toolchain for building binaries
    - Windows SDK for signtool.exe if signing
#>

[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",

    [string]$Version = "",

    [Alias("OutputDir")]
    [string]$OutputPath = ".\output",

    [bool]$Sign = ($Configuration -eq "Release"),

    [string]$CertificatePath = "",

    [SecureString]$CertificatePassword = $null,

    [switch]$SkipBuild
)

# Strict mode for better error handling
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

# Script paths
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Resolve-Path (Join-Path $ScriptDir "..\..")
$WixSourceDir = $ScriptDir
$StagingDir = Join-Path $ScriptDir "staging"

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

function Get-WixToolPath {
    param([string]$ToolName)

    # Check WIX environment variable (standard WiX 3.x installation)
    if ($env:WIX) {
        $toolPath = Join-Path $env:WIX "bin\$ToolName.exe"
        if (Test-Path $toolPath) {
            return $toolPath
        }
    }

    # Check PATH
    $tool = Get-Command $ToolName -ErrorAction SilentlyContinue
    if ($tool) {
        return $tool.Source
    }

    # Check common installation paths
    $commonPaths = @(
        "${env:ProgramFiles(x86)}\WiX Toolset v3.11\bin\$ToolName.exe",
        "${env:ProgramFiles(x86)}\WiX Toolset v3.14\bin\$ToolName.exe",
        "${env:ProgramFiles}\WiX Toolset v3.11\bin\$ToolName.exe",
        "${env:ProgramFiles}\WiX Toolset v3.14\bin\$ToolName.exe"
    )

    foreach ($path in $commonPaths) {
        if (Test-Path $path) {
            return $path
        }
    }

    throw "$ToolName.exe not found. Please install WiX Toolset 3.x or set the WIX environment variable."
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

    $cargoArgs = @("build", "--workspace", "-p", "flight-service", "-p", "flight-cli")
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

function Prepare-StagingDirectory {
    Write-Step "Preparing staging directory"

    # Clean and create staging directory
    if (Test-Path $StagingDir) {
        Remove-Item $StagingDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $StagingDir -Force | Out-Null

    # Create subdirectories
    $dirs = @("bin", "config")
    foreach ($dir in $dirs) {
        New-Item -ItemType Directory -Path (Join-Path $StagingDir $dir) -Force | Out-Null
    }

    Write-Success "Staging directory prepared at $StagingDir"
}

function Stage-Files {
    param([string]$BinDir)

    Write-Step "Staging files for installer"

    # Stage binaries
    $binaries = @(
        @{ Name = "flightd.exe"; Source = "flight-service.exe" },
        @{ Name = "flightctl.exe"; Source = "flight-cli.exe" }
    )

    foreach ($binary in $binaries) {
        # Try the actual binary name first, then the crate name
        $src = Join-Path $BinDir $binary.Name
        if (-not (Test-Path $src)) {
            $src = Join-Path $BinDir $binary.Source
        }

        $dst = Join-Path $StagingDir "bin\$($binary.Name)"

        if (Test-Path $src) {
            Copy-Item $src $dst -Force
            Write-Success "Staged $($binary.Name)"
        }
        else {
            throw "Binary not found: $($binary.Name) or $($binary.Source) in $BinDir"
        }
    }

    # Stage configuration files
    $configDir = Join-Path $StagingDir "config"

    # Create default config.toml
    $defaultConfig = @"
# Flight Hub Configuration
# See documentation at https://flight-hub.dev/docs for all options

[general]
# Log level: trace, debug, info, warn, error
log_level = "info"

[scheduler]
# Target processing frequency in Hz (default: 250)
frequency = 250

[ffb]
# Enable force feedback
enabled = true
# Safety envelope enabled
safety_enabled = true
"@
    Set-Content -Path (Join-Path $configDir "config.toml") -Value $defaultConfig -Encoding UTF8
    Write-Success "Created config.toml"

    # Create default profile
    $defaultProfile = @"
# Default Flight Hub Profile
# Customize this profile for your flight control setup

[profile]
name = "Default"
description = "Default Flight Hub profile"

[axes]
# Axis mappings - add your device axes here
# Example:
# [axes.pitch]
# device = "joystick"
# axis = 1
# curve = "linear"

[buttons]
# Button mappings - add your device buttons here
"@
    Set-Content -Path (Join-Path $configDir "default.profile.toml") -Value $defaultProfile -Encoding UTF8
    Write-Success "Created default.profile.toml"

    Write-Success "File staging complete"
}

function Invoke-Candle {
    param(
        [string]$CandlePath,
        [string]$Version,
        [string]$BinDir
    )

    Write-Step "Compiling WiX sources (candle.exe)"

    $objDir = Join-Path $StagingDir "obj"
    New-Item -ItemType Directory -Path $objDir -Force | Out-Null

    $wixVars = @(
        "-dBinDir=$StagingDir\bin",
        "-dConfigDir=$StagingDir\config",
        "-dVersion=$Version"
    )

    $objFiles = @()

    foreach ($source in $WixSources) {
        $sourcePath = Join-Path $WixSourceDir $source
        $objFile = Join-Path $objDir ([System.IO.Path]::GetFileNameWithoutExtension($source) + ".wixobj")
        $objFiles += $objFile

        $candleArgs = @("-nologo") + $wixVars + @("-out", $objFile, $sourcePath)

        Write-Host "  Compiling: $source"
        & $CandlePath @candleArgs
        if ($LASTEXITCODE -ne 0) {
            throw "candle.exe failed for $source with exit code $LASTEXITCODE"
        }
    }

    Write-Success "WiX compilation complete"
    return $objFiles
}

function Invoke-Light {
    param(
        [string]$LightPath,
        [string[]]$ObjectFiles,
        [string]$OutputFile
    )

    Write-Step "Linking MSI package (light.exe)"

    # WiX UI and Util extensions
    $extensions = @(
        "-ext", "WixUIExtension",
        "-ext", "WixUtilExtension"
    )

    $lightArgs = @("-nologo") + $extensions + @(
        "-cultures:en-us",
        "-out", $OutputFile
    ) + $ObjectFiles

    Write-Host "  Output: $OutputFile"
    & $LightPath @lightArgs
    if ($LASTEXITCODE -ne 0) {
        throw "light.exe failed with exit code $LASTEXITCODE"
    }

    Write-Success "MSI package created"
    return $OutputFile
}

function Sign-MsiPackage {
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
    $signtool = $null
    $signtoolCmd = Get-Command "signtool" -ErrorAction SilentlyContinue
    if ($signtoolCmd) {
        $signtool = $signtoolCmd.Source
    }
    else {
        # Search Windows SDK paths
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
        "/du", "https://github.com/EffortlessMetrics/OpenFlight"
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

function New-ChecksumFile {
    param(
        [string]$MsiPath
    )

    Write-Step "Generating checksum file"

    $checksumFile = "$MsiPath.sha256"
    $hash = Get-FileHash -Path $MsiPath -Algorithm SHA256

    $checksumContent = "$($hash.Hash.ToLower())  $(Split-Path -Leaf $MsiPath)"
    Set-Content -Path $checksumFile -Value $checksumContent -Encoding ASCII

    Write-Success "Checksum file created: $checksumFile"
    return $checksumFile
}

function Remove-StagingDirectory {
    Write-Step "Cleaning up"

    if (Test-Path $StagingDir) {
        Remove-Item $StagingDir -Recurse -Force
        Write-Success "Removed staging directory"
    }
}

# ============================================
# Main Build Process
# ============================================

try {
    Write-Host @"

================================================================================
              Flight Hub Windows Installer Build Script (WiX 3.x)
================================================================================

"@ -ForegroundColor Magenta

    # Validate WiX installation
    $candlePath = Get-WixToolPath "candle"
    $lightPath = Get-WixToolPath "light"
    Write-Success "Found WiX Toolset:"
    Write-Host "  candle.exe: $candlePath"
    Write-Host "  light.exe:  $lightPath"

    # Get version
    if (-not $Version) {
        $Version = Get-VersionFromCargo
    }
    Write-Host "`nBuild Configuration:" -ForegroundColor White
    Write-Host "  Version:       $Version"
    Write-Host "  Configuration: $Configuration"
    Write-Host "  Sign MSI:      $Sign"

    # Build Rust binaries (unless skipped)
    if (-not $SkipBuild) {
        Build-RustBinaries -Config $Configuration
    }
    else {
        Write-Warning "Skipping Rust build (using existing binaries)"
    }

    # Get binary directory
    $binDir = Get-BinaryDir -Config $Configuration
    Write-Host "  Binary Dir:    $binDir"

    # Ensure output directory exists
    if (-not (Test-Path $OutputPath)) {
        New-Item -ItemType Directory -Path $OutputPath -Force | Out-Null
    }
    $OutputPath = Resolve-Path $OutputPath

    # Prepare staging
    Prepare-StagingDirectory

    # Stage files
    Stage-Files -BinDir $binDir

    # Compile WiX sources
    $objFiles = Invoke-Candle -CandlePath $candlePath -Version $Version -BinDir $binDir

    # Link MSI
    $msiName = "FlightHub-$Version.msi"
    $msiPath = Join-Path $OutputPath $msiName
    Invoke-Light -LightPath $lightPath -ObjectFiles $objFiles -OutputFile $msiPath

    # Sign if requested
    if ($Sign -and $CertificatePath) {
        Sign-MsiPackage -MsiPath $msiPath -CertPath $CertificatePath -CertPassword $CertificatePassword
    }
    elseif ($Sign) {
        Write-Warning "Signing requested but no certificate provided. MSI is unsigned."
    }

    # Generate checksum
    $checksumFile = New-ChecksumFile -MsiPath $msiPath

    # Cleanup
    Remove-StagingDirectory

    # Summary
    $msiSize = (Get-Item $msiPath).Length / 1MB

    Write-Host @"

================================================================================
                            Build Complete!
================================================================================

MSI Package:  $msiPath
Size:         $([math]::Round($msiSize, 2)) MB
Version:      $Version
Signed:       $(if ($Sign -and $CertificatePath) { "Yes" } else { "No" })
Checksum:     $checksumFile

To install:
  msiexec /i "$msiPath" /l*v install.log

To install silently:
  msiexec /i "$msiPath" /qn /l*v install.log

"@ -ForegroundColor Green

}
catch {
    Write-Error $_.Exception.Message
    Write-Host $_.ScriptStackTrace -ForegroundColor Red
    exit 1
}
