---
doc_id: DOC-TROUBLESHOOTING
kind: how-to
area: infra
status: active
links:
  requirements: ["REQ-9"]
  tasks: []
  adrs: []
---

# Flight Hub Troubleshooting Guide

This guide helps resolve common issues with Flight Hub installation, configuration, and simulator integration.

## Installation Issues

### Windows Installation Problems

**Issue**: MSI installer fails with permission errors
**Solution**: 
1. Ensure you're installing to user directory (not Program Files)
2. Run installer as current user (not administrator)
3. Check Windows Defender hasn't quarantined the installer

**Issue**: "Application failed to start" after installation
**Solution**:
1. Install Microsoft Visual C++ Redistributable
2. Check Windows Event Viewer for specific error details
3. Try running in Safe Mode (see below)

### Linux Installation Problems

**Issue**: Systemd service fails to start
**Solution**:
1. Check service status: `systemctl --user status flight-hub`
2. Verify executable permissions: `ls -la /usr/local/bin/flight-hub`
3. Check logs: `journalctl --user -u flight-hub`

**Issue**: Permission denied errors
**Solution**:
1. Ensure installation was done without sudo
2. Check user has access to required directories
3. Verify udev rules are installed correctly

## Permission Issues

### HID Device Access (Linux)

**Symptoms**:
- Error: "Permission denied accessing /dev/hidraw0"
- Devices not detected despite being plugged in
- Works as root but not as regular user

**Solutions**:

1. **Add user to input group**
   ```bash
   sudo usermod -a -G input $USER
   ```
   **Important**: Log out and log back in for this to take effect.

2. **Verify group membership**
   ```bash
   groups
   ```
   You should see `input` in the list.

3. **Check udev rules are installed**
   ```bash
   ls -la /etc/udev/rules.d/99-flight-hub.rules
   ```
   If missing, install them:
   ```bash
   sudo cp /usr/share/flight-hub/99-flight-hub.rules /etc/udev/rules.d/
   sudo udevadm control --reload-rules
   sudo udevadm trigger
   ```

4. **Check device permissions**
   ```bash
   ls -la /dev/hidraw*
   ```
   Should show group `input` with `rw` permissions:
   ```
   crw-rw---- 1 root input 239, 0 Jan  1 12:00 /dev/hidraw0
   ```

5. **Manual permission fix (temporary)**
   ```bash
   sudo chmod 666 /dev/hidraw0
   ```
   Note: This resets on reboot. Use udev rules for permanent fix.

### Configuration Directory Access

**Symptoms**:
- Error: "Cannot write to configuration directory"
- Settings not saved
- Profiles not loading

**Solutions**:

1. **Check directory ownership**
   ```bash
   # Linux
   ls -la ~/.config/flight-hub
   
   # Windows (PowerShell)
   Get-Acl "$env:APPDATA\FlightHub"
   ```

2. **Fix ownership (Linux)**
   ```bash
   sudo chown -R $USER:$USER ~/.config/flight-hub
   ```

3. **Fix permissions (Linux)**
   ```bash
   chmod -R 755 ~/.config/flight-hub
   ```

4. **Reset configuration directory**
   ```bash
   # Backup first
   mv ~/.config/flight-hub ~/.config/flight-hub.backup
   
   # Restart Flight Hub to recreate
   flightctl service restart
   ```

### Simulator Integration Permissions

**MSFS (Windows)**:
- Flight Hub needs write access to `%APPDATA%\Microsoft Flight Simulator\`
- This is normally available to all users
- If denied, check folder permissions in Properties → Security

**X-Plane**:
- Flight Hub needs write access to X-Plane preferences folder
- Check `X-Plane 12/Output/preferences/` permissions

**DCS**:
- Export.lua requires write access to `Saved Games\DCS\Scripts\`
- This folder is user-owned and should be accessible
- If using Wine/Proton, check Wine prefix permissions

## Simulator Integration Issues

### Microsoft Flight Simulator (MSFS)

**Issue**: Flight Hub cannot connect to MSFS
**Solutions**:
1. Ensure MSFS is fully loaded (not just main menu)
2. Check SimConnect is enabled in MSFS settings
3. Verify Windows Firewall isn't blocking connections
4. Restart both MSFS and Flight Hub

**Issue**: Control inputs not working in MSFS
**Solutions**:
1. Check Flight Hub has disabled MSFS curves (UserCfg.opt)
2. Verify axis assignments in Flight Hub match MSFS
3. Test with default aircraft first
4. Check for conflicting input software

**Issue**: MSFS crashes when Flight Hub connects
**Solutions**:
1. Update MSFS to latest version
2. Disable other SimConnect applications temporarily
3. Check MSFS Community folder for conflicting addons
4. Try Flight Hub Safe Mode

### X-Plane

**Issue**: No data received from X-Plane
**Solutions**:
1. Verify UDP port 49000 is not blocked
2. Check X-Plane network settings allow UDP
3. Ensure DataRef output is enabled
4. Try different UDP port in Flight Hub settings

**Issue**: X-Plane plugin not loading
**Solutions**:
1. Check plugin is in correct directory: `X-Plane/Resources/plugins/FlightHub/`
2. Verify plugin file matches X-Plane version (32/64-bit)
3. Check X-Plane Log.txt for plugin errors
4. Ensure plugin dependencies are met

**Issue**: Controls feel different after Flight Hub installation
**Solutions**:
1. Check X-Plane response curves are disabled
2. Adjust Flight Hub curve settings
3. Verify axis calibration in Flight Hub
4. Compare with X-Plane's built-in curves temporarily

### DCS World

**Issue**: Export.lua not working
**Solutions**:
1. Verify Export.lua is in correct location: `Saved Games/DCS/Scripts/`
2. Check DCS scripting is enabled
3. Review DCS.log for Lua errors
4. Ensure Export.lua syntax is correct

**Issue**: No data in multiplayer
**Solutions**:
1. This is expected - many features are blocked in MP for integrity
2. Check Flight Hub UI shows "Multiplayer Session" banner
3. Verify single-player mode works correctly
4. Review multiplayer restrictions in documentation

**Issue**: DCS performance impact
**Solutions**:
1. Reduce Export.lua update rate
2. Disable unnecessary telemetry features
3. Check for other Export.lua conflicts
4. Monitor DCS frame rate and adjust accordingly

## Real-Time Scheduling Issues

### RT Not Enabled (Windows)

**Symptoms**: 
- `flightctl status` shows "RT Scheduling: Disabled" or "MMCSS: Not registered"
- Higher than expected input latency or jitter
- Warning: "MMCSS registration failed"

**Solutions**:

1. **Check Windows Audio Service**
   ```cmd
   sc query audiosrv
   ```
   MMCSS depends on the Windows Audio service. Ensure it's running:
   ```cmd
   net start audiosrv
   ```

2. **Check for MMCSS conflicts**
   - Some audio applications (DAWs, audio interfaces) may have exclusive MMCSS access
   - Close other audio-intensive applications
   - Check if any application is using "Pro Audio" MMCSS task

3. **Verify MMCSS is enabled**
   - Open Registry Editor (`regedit`)
   - Navigate to `HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Multimedia\SystemProfile`
   - Ensure `SystemResponsiveness` is set to a low value (0-20)

4. **Check thread priority elevation**
   - Flight Hub falls back to `SetThreadPriority` if MMCSS fails
   - This still provides good performance but slightly higher jitter
   - Check logs for: "Elevated thread priority via SetThreadPriority"

5. **Run as administrator (temporary test)**
   - If RT works as admin but not as user, there may be a policy restriction
   - Check Group Policy for thread priority restrictions

### RT Not Enabled (Linux)

**Symptoms**:
- `flightctl status` shows "RT Scheduling: Disabled"
- Warning: "rtkit failed" or "sched_setscheduler failed"
- Higher than expected input latency or jitter

**Solutions**:

1. **Install and enable rtkit**
   ```bash
   # Debian/Ubuntu
   sudo apt install rtkit
   sudo systemctl enable rtkit-daemon
   sudo systemctl start rtkit-daemon
   
   # Fedora
   sudo dnf install rtkit
   sudo systemctl enable rtkit-daemon
   sudo systemctl start rtkit-daemon
   ```

2. **Check rtkit status**
   ```bash
   systemctl status rtkit-daemon
   ```

3. **Configure limits.conf**
   If rtkit is unavailable, configure RT limits manually:
   ```bash
   sudo nano /etc/security/limits.conf
   ```
   Add these lines (replace `yourusername`):
   ```
   yourusername    soft    rtprio    99
   yourusername    hard    rtprio    99
   yourusername    soft    memlock   unlimited
   yourusername    hard    memlock   unlimited
   ```
   **Log out and log back in** for changes to take effect.

4. **Verify limits are applied**
   ```bash
   ulimit -r    # Should show 99
   ulimit -l    # Should show unlimited
   ```

5. **Check for kernel restrictions**
   Some hardened kernels restrict RT scheduling. Check:
   ```bash
   cat /proc/sys/kernel/sched_rt_runtime_us
   ```
   If this is -1, RT is unrestricted. If it's a positive number, RT processes are limited.

6. **Use the setup script**
   ```bash
   sudo scripts/setup-linux-rt.sh
   ```

### Verifying RT is Working

After applying fixes, verify RT scheduling:

```bash
flightctl status --verbose
```

**Windows expected output**:
```
RT Scheduling:
  MMCSS: Registered (task: Games)
  Thread Priority: TIME_CRITICAL
  High-Res Timer: Enabled
```

**Linux expected output**:
```
RT Scheduling:
  Method: rtkit (or sched_setscheduler)
  Policy: SCHED_FIFO
  Priority: 10
  mlockall: Success
```

## Force Feedback Issues

### No FFB Detected

**Symptoms**:
- `flightctl devices` shows device without `[FFB]` indicator
- FFB device works in other applications but not Flight Hub
- Error: "No FFB-capable device found"

**Solutions**:

1. **Verify device supports DirectInput FFB**
   - Open Windows Game Controllers (`joy.cpl`)
   - Select your device → Properties → Settings
   - Look for "Force Feedback" or "Effects" tab
   - If no FFB tab exists, the device may not support DirectInput FFB

2. **Check device drivers**
   - Ensure manufacturer drivers are installed (not just Windows generic)
   - Update to latest driver version
   - Some devices require specific driver versions for FFB

3. **Test FFB in Windows**
   - In Game Controllers → Properties → Test
   - Click "Test Force Feedback" or similar
   - If this doesn't work, the issue is with the device/driver, not Flight Hub

4. **Check USB connection**
   - FFB devices often require more USB power
   - Try a powered USB hub or direct motherboard port
   - Avoid USB extension cables

5. **Verify DirectInput is available**
   - Some devices use proprietary FFB protocols
   - Check if manufacturer provides DirectInput compatibility mode

6. **Check Flight Hub FFB status**
   ```cmd
   flightctl ffb status
   ```
   Look for device detection and capability information.

### FFB Device Detected but No Output

**Symptoms**:
- Device shows `[FFB]` in device list
- No forces felt during operation
- `flightctl ffb status` shows "State: Disabled" or "State: Faulted"

**Solutions**:

1. **Enable FFB**
   ```cmd
   flightctl ffb enable
   ```

2. **Check safety state**
   ```cmd
   flightctl ffb status
   ```
   If state is "Faulted", check for fault reasons and clear:
   ```cmd
   flightctl ffb clear
   ```

3. **Verify simulator connection**
   - FFB requires active simulator telemetry
   - Check `safe_for_ffb` flag is true
   - Ensure simulator is running and connected

4. **Check strength settings**
   ```cmd
   flightctl ffb strength 100
   ```

5. **Review blackbox for faults**
   ```cmd
   flightctl blackbox show --filter ffb --last 5m
   ```

6. **Test with manual effect**
   ```cmd
   flightctl ffb test
   ```
   This sends a test effect to verify basic FFB communication.

### FFB Cuts Out or Faults Frequently

**Symptoms**:
- FFB works then suddenly stops
- Frequent "USB stall" or "write failure" messages
- State frequently changes to "Faulted"

**Solutions**:

1. **Check USB connection quality**
   - Use a high-quality USB cable
   - Connect directly to motherboard USB port
   - Avoid USB hubs (especially unpowered ones)

2. **Check for USB power issues**
   - FFB devices draw significant power
   - Try a powered USB hub
   - Check if other USB devices cause issues

3. **Update device firmware**
   - Check manufacturer website for firmware updates
   - Some FFB issues are fixed in firmware

4. **Review fault log**
   ```cmd
   flightctl ffb faults
   ```
   Look for patterns (specific times, conditions, etc.)

5. **Reduce FFB update rate**
   - High update rates can overwhelm some devices
   - Try reducing in settings

6. **Check for driver conflicts**
   - Disable other input management software
   - Check for conflicting DirectInput hooks

## Performance Issues

### High CPU Usage

**Symptoms**: Flight Hub using >5% CPU constantly
**Solutions**:
1. Check for runaway processes in Task Manager
2. Reduce update rates in settings
3. Disable unnecessary features (panels, tactile)
4. Update to latest Flight Hub version

### High Memory Usage

**Symptoms**: Flight Hub using >200MB RAM
**Solutions**:
1. Restart Flight Hub service
2. Check for memory leaks in logs
3. Reduce blackbox recording duration
4. Disable debug logging if enabled

### Input Lag or Jitter

**Symptoms**: Noticeable delay or inconsistency in controls
**Solutions**:
1. Check system meets minimum requirements
2. Close unnecessary background applications
3. Verify USB devices are on dedicated controllers
4. Try different USB ports (avoid hubs)
5. Check Windows power management settings

## Network and Connectivity Issues

### Firewall Problems

**Issue**: Windows Firewall blocking connections
**Solutions**:
1. Add Flight Hub to Windows Firewall exceptions
2. Allow Flight Hub through private networks
3. Check third-party firewall software
4. Temporarily disable firewall for testing

### Port Conflicts

**Issue**: "Port already in use" errors
**Solutions**:
1. Check what's using the port: `netstat -an | findstr :49000`
2. Change port in Flight Hub settings
3. Restart conflicting applications
4. Reboot system if necessary

### Antivirus Interference

**Issue**: Antivirus blocking Flight Hub
**Solutions**:
1. Add Flight Hub directory to antivirus exclusions
2. Whitelist Flight Hub executable
3. Check quarantine for Flight Hub files
4. Temporarily disable real-time protection for testing

## Configuration Issues

### Profile Problems

**Issue**: Profiles not loading or applying
**Solutions**:
1. Check profile JSON syntax is valid
2. Verify profile file permissions
3. Review Flight Hub logs for profile errors
4. Try with default profile first

**Issue**: Aircraft auto-detection not working
**Solutions**:
1. Verify simulator is properly detected
2. Check aircraft identification in logs
3. Ensure profile naming matches aircraft
4. Test manual profile selection

### Calibration Issues

**Issue**: Axis calibration seems wrong
**Solutions**:
1. Recalibrate in Flight Hub settings
2. Check for hardware issues (potentiometer wear)
3. Verify axis assignment is correct
4. Test with different USB port

## Safe Mode and Recovery

### Starting in Safe Mode

When Flight Hub has issues, start in Safe Mode:

**Windows**: 
```cmd
flight-hub.exe --safe
```

**Linux**:
```bash
flight-hub --safe
```

Safe Mode disables:
- Panel integrations
- Plugin system
- Tactile feedback
- Advanced features

Only basic axis processing remains active.

### Complete Reset

To completely reset Flight Hub configuration:

1. **Stop Flight Hub service**
2. **Backup important profiles** (optional)
3. **Delete configuration directory**:
   - Windows: `%APPDATA%\FlightHub`
   - Linux: `~/.config/flight-hub`
4. **Restart Flight Hub**
5. **Run first-time setup wizard**

### Reverting Simulator Changes

If you need to completely remove Flight Hub's simulator integration:

- **MSFS**: [Follow MSFS revert steps](./integration/msfs.md#revert-steps)
- **X-Plane**: [Follow X-Plane revert steps](./integration/xplane.md#revert-steps)  
- **DCS**: [Follow DCS revert steps](./integration/dcs.md#revert-steps)

## Diagnostic Information

### Collecting Logs

Flight Hub logs are located:
- **Windows**: `%APPDATA%\FlightHub\logs\`
- **Linux**: `~/.local/share/flight-hub/logs/`

Important log files:
- `flight-hub.log` - Main application log
- `axis-engine.log` - Real-time axis processing
- `integration.log` - Simulator integration events

### System Information

When reporting issues, include:
1. Flight Hub version
2. Operating system and version
3. Simulator version(s)
4. Hardware configuration (controllers, etc.)
5. Relevant log excerpts
6. Steps to reproduce the issue

### Performance Monitoring

Check Flight Hub performance:
1. Open Task Manager / System Monitor
2. Monitor Flight Hub CPU and memory usage
3. Check for consistent high usage patterns
4. Note any correlation with simulator events

## Getting Help

### Self-Service Resources

1. **Documentation**: Check the integration guides for your simulator
2. **FAQ**: Review common questions and answers
3. **Community Forums**: Search existing discussions
4. **GitHub Issues**: Check for known issues and workarounds

### Contacting Support

When contacting support, provide:
1. **Clear problem description**
2. **Steps to reproduce**
3. **System information**
4. **Log files** (recent entries)
5. **Screenshots** (if UI-related)

### Emergency Recovery

If Flight Hub is completely broken:
1. **Uninstall Flight Hub**
2. **Follow simulator revert steps**
3. **Clean install Flight Hub**
4. **Restore backed-up profiles**
5. **Reconfigure step-by-step**

---

**Last Updated**: Based on Flight Hub specification and common user issues
**Version**: 1.0 - Initial troubleshooting guide