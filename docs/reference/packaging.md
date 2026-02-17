# Packaging Reference

This document describes the packaging structure, installed files, and service configuration for Flight Hub on supported platforms.

## Service Names

| Component | Name |
|-----------|------|
| Service binary | `flightd` |
| CLI binary | `flightctl` |
| Systemd service (Linux) | `flightd.service` |
| Windows service | N/A (user-mode application) |

## Linux (Debian/Ubuntu)

### Package Information

| Field | Value |
|-------|-------|
| Package name | `flight-hub` |
| Architecture | amd64 |
| Dependencies | libc6 (>= 2.31), libdbus-1-3 |
| Recommends | rtkit |

### Installed Files

| Path | Description |
|------|-------------|
| `/usr/bin/flightd` | Flight Hub service daemon |
| `/usr/bin/flightctl` | Command-line interface |
| `/usr/lib/systemd/user/flightd.service` | Systemd user service unit |
| `/usr/share/flight-hub/99-flight-hub.rules` | Udev rules (copied to /etc by postinst) |
| `/usr/share/flight-hub/setup-linux-rt.sh` | RT scheduling setup script |

### Configuration Paths

| Path | Description |
|------|-------------|
| `~/.config/flight-hub/` | User configuration directory |
| `~/.local/share/flight-hub/` | User data directory |
| `~/.local/share/flight-hub/logs/` | Log files |

### Udev Rules

The package installs udev rules that grant the `input` group access to:
- All hidraw devices
- USB HID devices
- Vendor-specific devices (Thrustmaster, Logitech, VKB, Virpil, Winwing)

### Post-Installation

The postinst script:
1. Adds the installing user to the `input` group
2. Copies udev rules to `/etc/udev/rules.d/`
3. Reloads udev rules
4. Displays installation instructions

**Note:** The postinst does NOT enable or start the systemd user service. This is intentional - postinst runs as root and cannot interact with user sessions. Users must enable the service manually after installation.

### Enabling/Disabling the User Service

After installing the .deb package:

```bash
# Enable and start the service
systemctl --user daemon-reload
systemctl --user enable --now flightd.service

# Check status
systemctl --user status flightd.service

# View logs
journalctl --user -u flightd.service -f

# Disable auto-start
systemctl --user disable flightd.service

# Stop the service
systemctl --user stop flightd.service
```

### Uninstallation

On purge, the postrm script:
1. Removes udev rules from `/etc/udev/rules.d/`
2. Reloads udev rules
3. Removes `/etc/flight-hub/` configuration directory

**Note:** The postrm does NOT disable or stop the user service (cannot access user sessions from dpkg context). If the service was running, it will fail after the binaries are removed. Users should stop the service before uninstalling:

```bash
systemctl --user stop flightd.service
systemctl --user disable flightd.service
sudo apt remove flight-hub
```

## Windows

### Installation Scope

Flight Hub uses **per-user installation** (no admin rights required).

### Installed Files

| Path | Description |
|------|-------------|
| `%LOCALAPPDATA%\FlightHub\bin\flightd.exe` | Service daemon |
| `%LOCALAPPDATA%\FlightHub\bin\flightctl.exe` | Command-line interface |
| `%LOCALAPPDATA%\FlightHub\` | Application directory |

### Configuration Paths

| Path | Description |
|------|-------------|
| `%APPDATA%\FlightHub\` | User configuration directory |
| `%APPDATA%\FlightHub\profiles\` | User profiles |
| `%APPDATA%\FlightHub\logs\` | Log files |

### Optional Integrations

The MSI installer offers optional features:

| Feature | Description |
|---------|-------------|
| MSFS | Microsoft Flight Simulator integration |
| X-Plane | X-Plane 11/12 plugin installation |
| DCS | DCS World Export.lua integration |

### Uninstallation (Windows)

Via Control Panel / Settings:
1. Open **Settings > Apps > Installed apps**
2. Find "Flight Hub" and click **Uninstall**

Via command line:
```powershell
# Find the product code
Get-WmiObject -Class Win32_Product | Where-Object { $_.Name -like "*FlightHub*" }

# Uninstall
msiexec /x {PRODUCT-CODE}
```

The uninstaller:
- Removes all installed binaries from `%LOCALAPPDATA%\FlightHub`
- Restores backed-up `Export.lua` for DCS (if DCS feature was installed)
- Removes X-Plane plugins (if X-Plane feature was installed)
- **Preserves** user configuration in `%APPDATA%\FlightHub\` by default

## Version Handling

### Debian Package

The `control` file in `installer/debian/` contains a placeholder version (`1.0.0`). During the release build, the workflow dynamically updates this:

```bash
sed -i "s/^Version: .*/Version: $VERSION/" "$PKG_DIR/DEBIAN/control"
```

### MSI Package

The WiX build script reads the version from `Cargo.toml` or accepts it as a parameter:

```powershell
.\build.ps1 -Version "1.2.3"
```

## Building Packages

### Linux (.deb)

The GitHub release workflow builds the deb package. For local testing:

```bash
# Build binaries
cargo build --release --workspace

# Create package structure manually
VERSION="1.0.0"
PKG_DIR="flight-hub_${VERSION}_amd64"
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/flight-hub"
mkdir -p "$PKG_DIR/usr/lib/systemd/user"

cp target/release/flightd "$PKG_DIR/usr/bin/"
cp target/release/flightctl "$PKG_DIR/usr/bin/"
cp installer/debian/control "$PKG_DIR/DEBIAN/"
cp installer/debian/postinst "$PKG_DIR/DEBIAN/"
cp installer/debian/postrm "$PKG_DIR/DEBIAN/"
cp installer/debian/99-flight-hub.rules "$PKG_DIR/usr/share/flight-hub/"
cp installer/debian/flightd.service "$PKG_DIR/usr/lib/systemd/user/"

chmod +x "$PKG_DIR/DEBIAN/postinst" "$PKG_DIR/DEBIAN/postrm"
sed -i "s/^Version: .*/Version: $VERSION/" "$PKG_DIR/DEBIAN/control"

dpkg-deb --build "$PKG_DIR"
```

### Windows (.msi)

```powershell
cd installer/wix
.\build.ps1 -Configuration Release
```

## Package Contents Verification

### Inspecting a .deb Package

```bash
# List contents
dpkg-deb -c flight-hub_*.deb

# Extract control files
dpkg-deb -e flight-hub_*.deb extracted/

# Compare control file (should only differ in version)
diff extracted/control installer/debian/control
```

### Inspecting an .msi Package

```powershell
# Use lessmsi or similar tool
lessmsi l FlightHub-*.msi
```

## Source Files

### Debian

| File | Description |
|------|-------------|
| `installer/debian/control` | Package metadata and dependencies |
| `installer/debian/postinst` | Post-installation script |
| `installer/debian/postrm` | Post-removal script |
| `installer/debian/99-flight-hub.rules` | Udev rules for HID access |
| `installer/debian/flightd.service` | Systemd user service unit |

### Windows

| File | Description |
|------|-------------|
| `installer/wix/Product.wxs` | WiX product definition |
| `installer/wix/Components.wxs` | Component definitions |
| `installer/wix/build.ps1` | Build script |
| `installer/wix/banner.bmp` | Installer banner image (493×58) |
| `installer/wix/dialog.bmp` | Installer dialog image (374×316) |
