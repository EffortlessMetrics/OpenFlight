# Linux Installation Guide

This guide walks you through installing Flight Hub on Linux systems.

## System Requirements

### Minimum Requirements
- **OS**: Ubuntu 22.04 LTS, Debian 12, Fedora 38, or compatible
- **Kernel**: 5.15 or later (for best HID support)
- **CPU**: Any x64 processor (Intel or AMD)
- **RAM**: 4 GB
- **Disk**: 100 MB free space
- **USB**: At least one USB 2.0 port for flight controllers

### Recommended Requirements
- **OS**: Ubuntu 24.04 LTS or Fedora 40
- **Kernel**: 6.1 or later
- **CPU**: Intel Core i5 / AMD Ryzen 5 or better
- **RAM**: 8 GB or more
- **USB**: USB 3.0 ports for reduced latency

### Supported Simulators
- X-Plane 11/12 (native Linux)
- DCS World (via Wine/Proton)
- Microsoft Flight Simulator (via Wine/Proton - experimental)

## Installation Methods

### Method 1: Debian/Ubuntu (.deb package)

#### Download and Install

```bash
# Download the latest .deb package
wget https://github.com/EffortlessMetrics/OpenFlight/releases/latest/download/flight-hub_1.0.0_amd64.deb

# Install the package
sudo dpkg -i flight-hub_1.0.0_amd64.deb

# Install any missing dependencies
sudo apt-get install -f
```

#### What the Package Installs

| Path | Description |
|------|-------------|
| `/usr/bin/flightd` | Flight Hub service daemon |
| `/usr/bin/flightctl` | Command-line interface |
| `/etc/udev/rules.d/99-flight-hub.rules` | HID device access rules |
| `/usr/share/flight-hub/` | Documentation and resources |

### Method 2: Build from Source

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone the repository
git clone https://github.com/EffortlessMetrics/OpenFlight.git
cd OpenFlight

# Build release binaries
cargo build --release --workspace

# Install binaries
sudo cp target/release/flightd /usr/local/bin/
sudo cp target/release/flightctl /usr/local/bin/

# Install udev rules
sudo cp infra/linux/99-flight-hub.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## Post-Installation Setup

### Step 1: Add User to Input Group

Flight Hub needs access to HID devices. Add your user to the `input` group:

```bash
sudo usermod -a -G input $USER
```

**Important**: You must log out and log back in for group changes to take effect.

### Step 2: Verify Group Membership

After logging back in:

```bash
groups
```

You should see `input` in the list.

### Step 3: Verify Installation

```bash
flightctl status
```

Expected output:
```
Flight Hub v1.0.0
Service: Running
RT Scheduling: Enabled (rtkit)
Simulators: None connected
```

### Step 4: Connect Flight Controllers

1. Plug in your flight controllers
2. Verify they're detected:
   ```bash
   flightctl devices
   ```

## Real-Time Scheduling Setup

Flight Hub uses real-time scheduling for low-latency axis processing. Linux requires additional configuration for unprivileged RT scheduling.

### Option 1: rtkit (Recommended)

rtkit allows unprivileged processes to request RT scheduling via D-Bus.

```bash
# Install rtkit (usually pre-installed on desktop systems)
sudo apt install rtkit    # Debian/Ubuntu
sudo dnf install rtkit    # Fedora

# Verify rtkit is running
systemctl status rtkit-daemon
```

Flight Hub will automatically use rtkit when available.

### Option 2: Configure limits.conf

If rtkit is unavailable, configure RT limits manually:

```bash
# Run the setup script
sudo scripts/setup-linux-rt.sh
```

Or manually edit `/etc/security/limits.conf`:

```bash
# Add these lines (replace 'yourusername' with your actual username)
yourusername    soft    rtprio    99
yourusername    hard    rtprio    99
yourusername    soft    memlock   unlimited
yourusername    hard    memlock   unlimited
```

**Important**: Log out and log back in for limits to take effect.

### Verify RT Status

```bash
flightctl status --verbose
```

Look for:
```
RT Scheduling:
  Method: rtkit
  Policy: SCHED_FIFO
  Priority: 10
  mlockall: Success
```

### If RT Scheduling Fails

If RT scheduling is unavailable, Flight Hub will continue to work at normal priority with a warning:

```
WARNING: RT scheduling unavailable. Running at normal priority.
         Axis processing may have higher jitter.
```

See [Troubleshooting: RT Not Enabled](../how-to/troubleshoot-common-issues.md#real-time-scheduling-issues) for solutions.

## Running as a Service

### Systemd User Service

Create a user service for automatic startup:

```bash
# Create service directory
mkdir -p ~/.config/systemd/user

# Create service file
cat > ~/.config/systemd/user/flight-hub.service << 'EOF'
[Unit]
Description=Flight Hub Service
After=graphical-session.target

[Service]
Type=simple
ExecStart=/usr/bin/flightd
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# Enable and start the service
systemctl --user daemon-reload
systemctl --user enable flight-hub
systemctl --user start flight-hub
```

### Check Service Status

```bash
systemctl --user status flight-hub
```

### View Logs

```bash
journalctl --user -u flight-hub -f
```

## Simulator Integration

### X-Plane (Native Linux)

X-Plane runs natively on Linux. See the [X-Plane Setup Guide](../explanation/integration/xplane.md) for configuration.

Quick setup:
1. Enable UDP Data Output in X-Plane settings
2. Configure output to `127.0.0.1:49000`
3. Enable required data groups (3, 4, 16, 17, 18, 21)

### DCS World (Wine/Proton)

DCS runs via Wine or Proton. The Export.lua integration works the same as on Windows:

1. Locate your DCS Saved Games folder (usually `~/.wine/drive_c/users/*/Saved Games/DCS/`)
2. Install Export.lua as described in the [DCS Setup Guide](../explanation/integration/dcs.md)

### MSFS (Wine/Proton - Experimental)

MSFS via Proton is experimental. SimConnect communication may require additional Wine configuration.

## Uninstallation

### Remove .deb Package

```bash
sudo apt remove flight-hub
```

### Remove from Source Install

```bash
sudo rm /usr/local/bin/flightd
sudo rm /usr/local/bin/flightctl
sudo rm /etc/udev/rules.d/99-flight-hub.rules
sudo udevadm control --reload-rules
```

### Remove Configuration

```bash
rm -rf ~/.config/flight-hub
rm -rf ~/.local/share/flight-hub
```

### Remove User Service

```bash
systemctl --user stop flight-hub
systemctl --user disable flight-hub
rm ~/.config/systemd/user/flight-hub.service
systemctl --user daemon-reload
```

## Troubleshooting

### Permission Denied Errors

```
Error: Permission denied accessing /dev/hidraw0
```

**Solution**:
1. Verify you're in the `input` group: `groups`
2. If not, add yourself: `sudo usermod -a -G input $USER`
3. Log out and log back in
4. Verify udev rules are installed: `ls /etc/udev/rules.d/99-flight-hub.rules`

### Device Not Detected

```bash
# Check if device is visible to the system
lsusb

# Check hidraw devices
ls -la /dev/hidraw*

# Check device permissions
stat /dev/hidraw0
```

### Service Won't Start

```bash
# Check service status
systemctl --user status flight-hub

# Check logs for errors
journalctl --user -u flight-hub --no-pager -n 50
```

For more issues, see the [Troubleshooting Guide](../how-to/troubleshoot-common-issues.md).

## Next Steps

1. [Configure simulator integration](#simulator-integration)
2. [Set up real-time scheduling](#real-time-scheduling-setup)
3. [Configure force feedback](./configure-ffb.md) (if you have FFB devices)

---

**Requirements**: 17.1 (Linux install guide)
