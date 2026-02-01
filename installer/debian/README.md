# Flight Hub Debian Package

This directory contains the Debian package configuration for Flight Hub.

## Building the Package

The package is built automatically by the release workflow. To build manually:

```bash
# From the repository root
VERSION="1.0.0"
PKG_DIR="flight-hub_${VERSION}_amd64"

# Create package structure
mkdir -p "$PKG_DIR/DEBIAN"
mkdir -p "$PKG_DIR/usr/bin"
mkdir -p "$PKG_DIR/etc/udev/rules.d"
mkdir -p "$PKG_DIR/usr/share/flight-hub"
mkdir -p "$PKG_DIR/usr/share/doc/flight-hub"

# Copy files
cp installer/debian/control "$PKG_DIR/DEBIAN/"
cp installer/debian/postinst "$PKG_DIR/DEBIAN/"
cp installer/debian/postrm "$PKG_DIR/DEBIAN/"
chmod +x "$PKG_DIR/DEBIAN/postinst" "$PKG_DIR/DEBIAN/postrm"

cp target/release/flightd "$PKG_DIR/usr/bin/"
cp target/release/flightctl "$PKG_DIR/usr/bin/"
chmod +x "$PKG_DIR/usr/bin/"*

cp installer/debian/99-flight-hub.rules "$PKG_DIR/usr/share/flight-hub/"
cp scripts/setup-linux-rt.sh "$PKG_DIR/usr/share/flight-hub/"

# Build package
dpkg-deb --build "$PKG_DIR"
```

## Package Contents

- `/usr/bin/flightd` - Flight Hub daemon
- `/usr/bin/flightctl` - Flight Hub CLI
- `/etc/udev/rules.d/99-flight-hub.rules` - udev rules for HID access
- `/usr/share/flight-hub/setup-linux-rt.sh` - RT scheduling setup script

## Dependencies

- `libc6 (>= 2.31)` - C library
- `libdbus-1-3` - D-Bus library (for rtkit integration)
- `rtkit` (recommended) - Real-time scheduling via D-Bus

## Post-Installation

The postinst script:
1. Adds the installing user to the `input` group
2. Installs udev rules for HID device access
3. Reloads udev rules

Users must log out and back in for group changes to take effect.

## Real-Time Scheduling

For optimal performance, run the RT setup script:

```bash
sudo /usr/share/flight-hub/setup-linux-rt.sh
```

This configures:
- `/etc/security/limits.conf` for rtprio and memlock
- Installs rtkit if not present
- Provides instructions for group membership
