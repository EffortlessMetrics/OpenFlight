# Flight Hub Windows Installer (WiX)

This directory contains the WiX Toolset v4 project for building the Flight Hub Windows MSI installer.

## Requirements

- [WiX Toolset v4](https://wixtoolset.org/) - Install via `dotnet tool install --global wix`
- Windows SDK (for signtool.exe if signing)
- Rust toolchain (for building binaries)
- PowerShell 5.1 or later

## Project Structure

```
installer/wix/
├── Product.wxs       # Main WiX source - package definition, features, custom actions
├── Components.wxs    # Component definitions - files, directories, registry
├── build.ps1         # Build script for creating the MSI
├── README.md         # This file
├── License.rtf       # License text for installer UI (to be created)
├── banner.bmp        # Installer banner image (to be created)
└── dialog.bmp        # Installer dialog background (to be created)
```

## Features

The installer supports the following features:

| Feature | ID | Level | Description |
|---------|-----|-------|-------------|
| **Core** | `Core` | 1 (Required) | Flight Hub binaries and configuration |
| **MSFS** | `MSFS` | 2 (Optional) | Microsoft Flight Simulator integration |
| **X-Plane** | `XPlane` | 2 (Optional) | X-Plane 11/12 integration |
| **DCS** | `DCS` | 2 (Optional) | DCS World integration |

### Feature Levels

- Level 1: Installed by default, cannot be deselected
- Level 2: Not installed by default, user must opt-in

## Building

### Basic Build

```powershell
.\build.ps1
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
| `-Sign` | $true for Release | Whether to sign the MSI |
| `-CertificatePath` | - | Path to code signing certificate |
| `-CertificatePassword` | - | Certificate password (SecureString) |
| `-OutputDir` | `.\output` | Output directory for MSI |

## Installation Behavior

### Per-User Install (Default)

The installer uses per-user scope by default (Requirement 9.3):
- Installs to `%LOCALAPPDATA%\FlightHub`
- No admin rights required
- User-specific configuration

### Custom Actions

The installer includes custom actions for simulator integration:

1. **DCS Integration**
   - Backs up existing `Export.lua` before modification
   - Installs `FlightHubExport.lua` to DCS Scripts folder
   - Restores original `Export.lua` on uninstall

2. **X-Plane Integration**
   - Installs Flight Hub plugin to X-Plane plugins folder
   - Removes plugin on uninstall

3. **Product Posture**
   - Displays product posture summary during installation

## Uninstallation

The uninstaller (Requirement 9.6):
- Removes all installed binaries
- Restores backed-up `Export.lua` for DCS
- Removes X-Plane plugins
- Preserves user configuration (optional)

## Requirements Traceability

| Requirement | Implementation |
|-------------|----------------|
| 9.1 | MSI package using WiX Toolset |
| 9.2 | Features: Core (required), MSFS, X-Plane, DCS (optional) |
| 9.3 | Per-user install scope (`Scope="perUser"`) |
| 9.4 | Product posture displayed via custom action |
| 9.5 | Sim integrations are opt-in (Level="2") |
| 9.6 | Uninstaller restores Export.lua, removes plugins |

## Development

### Adding New Components

1. Add component definition to `Components.wxs`
2. Reference component group in appropriate feature in `Product.wxs`
3. Update staging logic in `build.ps1` if needed

### Testing

```powershell
# Build debug version
.\build.ps1 -Configuration Debug

# Install for testing
msiexec /i output\FlightHub-x.y.z.msi /l*v install.log

# Uninstall
msiexec /x output\FlightHub-x.y.z.msi /l*v uninstall.log
```

### Debugging

Enable verbose logging during installation:
```powershell
msiexec /i FlightHub.msi /l*v install.log
```

## License

This installer is part of Flight Hub and is dual-licensed under Apache-2.0 and MIT.
