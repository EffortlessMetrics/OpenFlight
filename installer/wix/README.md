# Flight Hub Windows Installer (WiX 3.x)

This directory contains the WiX Toolset v3.x project for building the Flight Hub Windows MSI installer.

## Requirements

- [WiX Toolset v3.11+](https://wixtoolset.org/releases/) - Download and install the toolset
- Windows SDK (for signtool.exe if signing)
- Rust toolchain (for building binaries)
- PowerShell 5.1 or later

## Project Structure

```
installer/wix/
├── Product.wxs         # Main WiX source - package definition, features, UI
├── Components.wxs      # Component definitions - files, directories, registry
├── build.ps1           # PowerShell build script for creating the MSI
├── README.md           # This file
├── License.rtf         # License text for installer UI (MIT/Apache-2.0)
├── banner.bmp          # Installer banner image (493x58 pixels)
├── dialog.bmp          # Installer dialog background (493x312 pixels)
└── generate-images.py  # Script to regenerate placeholder images
```

## UI Images

The installer requires two BMP images for the WiX UI:

| Image | Dimensions | Purpose |
|-------|------------|---------|
| `banner.bmp` | 493x58 | Displayed at the top of installer dialog pages |
| `dialog.bmp` | 493x312 | Displayed on the left side of welcome/finish pages |

**Requirements:**
- Format: 24-bit BMP (uncompressed)
- The current images are solid-color placeholders
- For a professional release, replace with properly branded images

**Regenerating placeholders (optional):**
```powershell
# Requires: pip install pillow
python generate-images.py
```

## Features

The installer provides the following features:

| Feature | ID | Level | Description |
|---------|-----|-------|-------------|
| **Core** | `Core` | 1 (Required) | Flight Hub daemon (flightd.exe) and configuration |
| **CLI Tools** | `CLI` | 2 (Default) | Command-line interface (flightctl.exe) |
| **Add to PATH** | `PathEnv` | 2 (Optional) | Add bin directory to system PATH |

### Feature Levels

- Level 1: Required, cannot be deselected
- Level 2: Default selected, user can deselect

## Installation

### Install Location

The installer installs to `C:\Program Files\Flight Hub\` by default (per-machine installation).

Directory structure after installation:
```
C:\Program Files\Flight Hub\
├── bin\
│   ├── flightd.exe     # Main daemon
│   └── flightctl.exe   # CLI tool (if CLI feature selected)
├── config\
│   ├── config.toml     # Main configuration
│   └── default.profile.toml
└── logs\               # Log directory
```

### Windows Service

The installer registers `flightd.exe` as a Windows service named "FlightHub" that:
- Starts automatically on system boot
- Runs as LocalSystem
- Automatically restarts on failure (up to 2 times)

### Registry Entries

The installer creates the following registry entries:

```
HKLM\SOFTWARE\OpenFlight\Flight Hub
├── InstallPath     # Installation directory
├── Version         # Installed version
├── BinPath         # Path to executables
└── ConfigPath      # Path to configuration

HKLM\SYSTEM\CurrentControlSet\Services\FlightHub
├── ImagePath       # Service executable path
└── Parameters\
    ├── ConfigPath  # Configuration file path
    └── LogPath     # Log directory path
```

## Building

### Prerequisites

1. Install WiX Toolset v3.11 or later
2. Ensure `candle.exe` and `light.exe` are in PATH or set the `WIX` environment variable

### Basic Build

```powershell
.\build.ps1
```

This will:
1. Build Rust binaries (release mode)
2. Stage files for the installer
3. Compile WiX sources with candle.exe
4. Link MSI package with light.exe
5. Generate SHA256 checksum file

### Build with Custom Version

```powershell
.\build.ps1 -Version "1.2.3"
```

### Build with Signing

```powershell
.\build.ps1 -Sign $true -CertificatePath "path\to\cert.pfx"
```

### Build Options

| Parameter | Default | Description |
|-----------|---------|-------------|
| `-Configuration` | Release | Build configuration (Debug/Release) |
| `-Version` | From Cargo.toml | Version string for the installer |
| `-OutputPath` | `.\output` | Output directory for MSI |
| `-Sign` | $true for Release | Whether to sign the MSI |
| `-CertificatePath` | - | Path to code signing certificate |
| `-CertificatePassword` | - | Certificate password (SecureString) |
| `-SkipBuild` | $false | Skip Rust binary build |

### Output Files

After a successful build:
```
output\
├── FlightHub-x.y.z.msi        # MSI installer package
└── FlightHub-x.y.z.msi.sha256 # SHA256 checksum file
```

## Installation Commands

### Interactive Installation

```powershell
msiexec /i FlightHub-1.0.0.msi
```

### Silent Installation

```powershell
# Install all features
msiexec /i FlightHub-1.0.0.msi /qn

# Install without adding to PATH
msiexec /i FlightHub-1.0.0.msi /qn ADDLOCAL=Core,CLI

# Install to custom directory
msiexec /i FlightHub-1.0.0.msi /qn INSTALLFOLDER="D:\FlightHub"
```

### Installation with Logging

```powershell
msiexec /i FlightHub-1.0.0.msi /l*v install.log
```

### Uninstallation

```powershell
# Interactive
msiexec /x FlightHub-1.0.0.msi

# Silent
msiexec /x FlightHub-1.0.0.msi /qn
```

## Add/Remove Programs Entry

The installer creates an entry in Windows Add/Remove Programs with:
- Product name: Flight Hub
- Publisher: OpenFlight
- Version: (from installer)
- Help link: GitHub discussions
- Update info: GitHub releases page

## Troubleshooting

### WiX Not Found

If you see "candle.exe not found", ensure:
1. WiX Toolset is installed
2. Either add WiX bin directory to PATH, or
3. Set the `WIX` environment variable to the WiX installation directory

```powershell
# Example: Set WIX environment variable
$env:WIX = "C:\Program Files (x86)\WiX Toolset v3.11"
```

### Build Errors

Enable verbose WiX output:
```powershell
# Modify build.ps1 to remove -nologo and add -v for verbose output
```

### Installation Errors

Check the MSI log:
```powershell
msiexec /i FlightHub.msi /l*v install.log
```

Common issues:
- **Error 1925**: Insufficient privileges. Run as Administrator.
- **Error 1603**: Generic failure. Check install.log for details.

## Development

### Adding New Components

1. Add component definition to `Components.wxs`
2. Add component to appropriate ComponentGroup
3. Reference ComponentGroup in feature in `Product.wxs`

### Modifying Features

Edit the `<Feature>` elements in `Product.wxs`. Remember:
- Level 1 = Required (cannot deselect)
- Level 2 = Default selected
- Level 1000+ = Not installed by default

### Testing Changes

```powershell
# Build debug version (faster)
.\build.ps1 -Configuration Debug -SkipBuild

# Test installation
msiexec /i output\FlightHub-0.1.0.msi /l*v install.log

# Test uninstallation
msiexec /x output\FlightHub-0.1.0.msi /l*v uninstall.log
```

## License

This installer is part of Flight Hub and is dual-licensed under Apache-2.0 and MIT.
