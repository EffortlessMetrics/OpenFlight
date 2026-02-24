# Windows Installation Guide

This guide walks you through installing Flight Hub on Windows 10 or Windows 11.

## System Requirements

### Minimum Requirements
- **OS**: Windows 10 (version 1903 or later) or Windows 11
- **CPU**: Any x64 processor (Intel or AMD)
- **RAM**: 4 GB
- **Disk**: 100 MB free space
- **USB**: At least one USB 2.0 port for flight controllers

### Recommended Requirements
- **OS**: Windows 11 (latest updates)
- **CPU**: Intel Core i5 / AMD Ryzen 5 or better
- **RAM**: 8 GB or more
- **USB**: USB 3.0 ports for reduced latency

### Supported Simulators
- Microsoft Flight Simulator 2020/2024
- X-Plane 11/12
- DCS World 2.7+

## Installation Steps

### Step 1: Download the Installer

1. Download the latest Flight Hub MSI installer from the [releases page](https://github.com/EffortlessMetrics/OpenFlight/releases)
2. The file will be named `FlightHub-x.y.z-win64.msi` (where x.y.z is the version number)

### Step 2: Run the Installer

1. Double-click the downloaded MSI file
2. If Windows SmartScreen appears, click "More info" then "Run anyway"
   - The installer is signed with a valid code signing certificate
3. Follow the installation wizard:
   - Accept the license agreement
   - Review the [Product Posture](../product-posture.md) summary
   - Choose installation location (default: `%LOCALAPPDATA%\FlightHub`)

### Step 3: Select Components

The installer offers these components:

| Component | Description | Default |
|-----------|-------------|---------|
| **Core** (required) | Flight Hub service and CLI | ✅ Always installed |
| **MSFS Integration** | Microsoft Flight Simulator support | ☐ Optional |
| **X-Plane Integration** | X-Plane 11/12 support | ☐ Optional |
| **DCS Integration** | DCS World support | ☐ Optional |

Select the simulators you use. You can add or remove integrations later.

### Step 4: Complete Installation

1. Click "Install" to begin installation
2. Wait for the installation to complete
3. Click "Finish"

The Flight Hub service (`flightd`) will start automatically.

## Post-Installation Setup

### Verify Installation

Open a command prompt and run:

```cmd
flightctl status
```

You should see:
```
Flight Hub v1.0.0
Service: Running
RT Scheduling: Enabled (MMCSS)
Simulators: None connected
```

### Connect Your Flight Controllers

1. Plug in your flight controllers (joystick, throttle, pedals)
2. Run `flightctl devices` to see detected devices:
   ```
   Detected Devices:
     [1] VKB Gladiator NXT EVO (Right)
     [2] Thrustmaster TWCS Throttle
     [3] Thrustmaster T.Flight Rudder Pedals
   ```

### Configure Simulator Integration

See the per-simulator setup guides:
- [MSFS Setup Guide](../explanation/integration/msfs.md)
- [X-Plane Setup Guide](../explanation/integration/xplane.md)
- [DCS Setup Guide](../explanation/integration/dcs.md)
- [Ace Combat 7 (Experimental) Setup Guide](../explanation/integration/ac7.md)

## Real-Time Scheduling (MMCSS)

Flight Hub uses Windows Multimedia Class Scheduler Service (MMCSS) for real-time thread priority. This is enabled automatically and requires no configuration.

### Verify RT Status

```cmd
flightctl status --verbose
```

Look for:
```
RT Scheduling:
  MMCSS: Registered (task: Games)
  Thread Priority: TIME_CRITICAL
  High-Res Timer: Enabled
  Power Throttling: Disabled
```

### If MMCSS Registration Fails

MMCSS registration may fail on some systems. Flight Hub will continue to work but with slightly higher jitter. To troubleshoot:

1. Check Windows Audio service is running
2. Ensure no other application has exclusive MMCSS access
3. See [Troubleshooting: RT Not Enabled](../how-to/troubleshoot-common-issues.md#real-time-scheduling-issues)

## Uninstallation

### Using Windows Settings

1. Open **Settings** → **Apps** → **Installed apps**
2. Find "Flight Hub" in the list
3. Click the three dots menu → **Uninstall**
4. Follow the uninstall wizard

### Using Control Panel

1. Open **Control Panel** → **Programs** → **Programs and Features**
2. Find "Flight Hub" in the list
3. Click **Uninstall**

### What Gets Removed

The uninstaller will:
- Remove all Flight Hub binaries
- Remove the Flight Hub service
- Restore any backed-up simulator configuration files
- **Keep** your profiles and settings (in `%APPDATA%\FlightHub`)

To completely remove all data:
```cmd
rmdir /s /q "%APPDATA%\FlightHub"
```

## Troubleshooting

### Common Issues

**Issue**: "Windows protected your PC" message
**Solution**: Click "More info" → "Run anyway". The installer is signed but may not be recognized by SmartScreen initially.

**Issue**: Service fails to start
**Solution**: 
1. Check Windows Event Viewer for errors
2. Ensure Visual C++ Redistributable is installed
3. Try running `flightctl service start` manually

**Issue**: No devices detected
**Solution**:
1. Ensure devices are plugged in before starting Flight Hub
2. Check Device Manager for driver issues
3. Try different USB ports

For more issues, see the [Troubleshooting Guide](../how-to/troubleshoot-common-issues.md).

## Next Steps

1. [Configure your first profile](../how-to/setup-dev-env.md)
2. [Set up simulator integration](#configure-simulator-integration)
3. [Configure force feedback](./configure-ffb.md) (if you have FFB devices)

---

**Requirements**: 17.1 (Windows install guide)
