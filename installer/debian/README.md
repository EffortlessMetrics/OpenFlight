# Flight Hub Debian Package

This directory contains the Debian package configuration for Flight Hub.

## Files

| File | Description |
|------|-------------|
| `control` | Package metadata and dependencies |
| `postinst` | Post-installation script (POSIX-compliant) |
| `postrm` | Post-removal script (POSIX-compliant) |
| `99-flight-hub.rules` | udev rules for HID device access |
| `flightd.service` | Systemd user service unit |

## Building the Package

The package is built automatically by the release workflow. To build manually:

```bash
# From the repository root
VERSION="1.0.0"
PKG_DIR="flight-hub_${VERSION}_amd64"

# Create package structure
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/usr/share/flight-hub"
mkdir -p "$PKG_DIR/usr/lib/systemd/user"

# Copy control files
cp installer/debian/control "$PKG_DIR/DEBIAN/"
cp installer/debian/postinst "$PKG_DIR/DEBIAN/"
cp installer/debian/postrm "$PKG_DIR/DEBIAN/"
chmod +x "$PKG_DIR/DEBIAN/postinst" "$PKG_DIR/DEBIAN/postrm"

# Update version in control file
sed -i "s/{{VERSION}}/$VERSION/" "$PKG_DIR/DEBIAN/control"

# Copy binaries
cp target/release/flightd "$PKG_DIR/usr/bin/"
cp target/release/flightctl "$PKG_DIR/usr/bin/"
chmod +x "$PKG_DIR/usr/bin/"*

# Copy support files
cp installer/debian/99-flight-hub.rules "$PKG_DIR/usr/share/flight-hub/"
cp installer/debian/flightd.service "$PKG_DIR/usr/lib/systemd/user/"
cp scripts/setup-linux-rt.sh "$PKG_DIR/usr/share/flight-hub/"

# Build package
dpkg-deb --build "$PKG_DIR"
```

## Package Contents

| Path | Description |
|------|-------------|
| `/usr/bin/flightd` | Flight Hub service daemon |
| `/usr/bin/flightctl` | Command-line interface |
| `/usr/lib/systemd/user/flightd.service` | Systemd user service unit |
| `/usr/share/flight-hub/99-flight-hub.rules` | udev rules (copied to /etc by postinst) |
| `/usr/share/flight-hub/setup-linux-rt.sh` | RT scheduling setup script |

## Dependencies

- `libc6 (>= 2.31)` - C library
- `libdbus-1-3` - D-Bus library (for rtkit integration)
- `libudev1` - udev library (for HID device enumeration)
- `rtkit` (recommended) - Real-time scheduling via D-Bus

## Post-Installation

The postinst script:
1. Copies udev rules to `/etc/udev/rules.d/`
2. Reloads udev rules
3. Adds the installing user to the `input` group (if SUDO_USER is set)
4. Displays post-installation instructions

**Note:** The postinst does NOT enable or start the systemd user service.
This is intentional - postinst runs as root and cannot interact with user sessions.
Users must enable the service manually after installation.

## User Service Management

After installation, users should run:

```bash
# Reload systemd user daemon
systemctl --user daemon-reload

# Enable and start the service
systemctl --user enable --now flightd.service

# Check status
systemctl --user status flightd.service

# View logs
journalctl --user -u flightd.service -f
```

## Uninstallation

Before removing the package, users should stop the service:

```bash
systemctl --user stop flightd.service
systemctl --user disable flightd.service
sudo apt remove flight-hub
```

On purge (`apt purge flight-hub`), the postrm script removes:
- udev rules from `/etc/udev/rules.d/`
- System configuration directory `/etc/flight-hub/` (if exists)
- System data directory `/var/lib/flightd/` (if exists)

User data in `~/.config/flight-hub/` and `~/.local/share/flight-hub/` is preserved.

## Real-Time Scheduling

For optimal performance, run the RT setup script:

```bash
sudo /usr/share/flight-hub/setup-linux-rt.sh
```

This configures:
- `/etc/security/limits.conf` for rtprio and memlock
- Installs rtkit if not present
- Provides instructions for group membership

## Supported HID Devices

The udev rules grant access to devices from these vendors:
- Saitek/Logitech Flight (06a3): X52, X55, X56, Pro Flight panels
- Thrustmaster (044f): HOTAS Warthog, T.16000M, TCA, TWCS
- Logitech General (046d): Extreme 3D Pro, G940
- VKB (231d): Gladiator NXT, Gunfighter, STECS
- Virpil (3344): Alpha, Constellation, CM2/CM3
- Winwing (4098): Orion, F-16EX, F/A-18
- CH Products (068e): Pro Throttle, Fighterstick, Pro Pedals
